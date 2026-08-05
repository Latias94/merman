#![cfg(feature = "bindgen-smoke")]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[test]
fn generates_python_binding_from_cdylib_metadata() {
    let workspace_root = workspace_root();
    let cdylib = build_cdylib(&workspace_root);
    let out_dir = tempfile::tempdir().expect("create bindgen smoke tempdir");

    generate_python_bindings(&cdylib, out_dir.path());
    let python_files = generated_files_with_extension(out_dir.path(), "py");
    assert_eq!(
        python_files.len(),
        1,
        "expected exactly one generated Python binding file in {}",
        out_dir.path().display()
    );

    let generated = fs::read_to_string(&python_files[0]).expect("read generated Python binding");
    assert!(
        generated.contains("class MermanEngine"),
        "generated binding should expose MermanEngine"
    );
    assert!(
        generated.contains("class MermanReusableEngine"),
        "generated binding should expose MermanReusableEngine"
    );
    assert!(
        generated.contains("class MermanTextMeasurer"),
        "generated binding should expose MermanTextMeasurer"
    );
    assert!(
        generated.contains("class MermanTextMeasureRequest"),
        "generated binding should expose MermanTextMeasureRequest"
    );
    assert!(
        generated.contains("class MermanTextMeasureResult"),
        "generated binding should expose MermanTextMeasureResult"
    );
    assert!(
        generated.contains("class MermanTextMeasurementPhase"),
        "generated binding should expose MermanTextMeasurementPhase"
    );
    assert!(
        generated.contains("class MermanTextMeasurementOperation"),
        "generated binding should expose MermanTextMeasurementOperation"
    );
    assert!(
        generated.contains("CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET"),
        "generated binding should expose operation 17"
    );
    assert!(
        generated.contains("RAW_B_BOX_HEIGHT"),
        "generated binding should expose operation 18"
    );
    assert!(
        generated.contains("class MermanTextMeasurementResultKind"),
        "generated binding should expose MermanTextMeasurementResultKind"
    );
    assert!(
        generated.contains("class MermanTextWrapMode"),
        "generated binding should expose MermanTextWrapMode"
    );
    assert!(
        generated.contains("class MermanDiagramFamilyCapability"),
        "generated binding should expose MermanDiagramFamilyCapability"
    );
    assert!(
        generated.contains("class MermanLintRuleCatalogEntry"),
        "generated binding should expose MermanLintRuleCatalogEntry"
    );
    assert!(
        generated.contains("class MermanAsciiCapability"),
        "generated binding should expose MermanAsciiCapability"
    );
    assert!(
        generated.contains("class MermanAsciiCapabilityEvidence"),
        "generated binding should expose MermanAsciiCapabilityEvidence"
    );
    assert!(
        generated.contains("def render_svg"),
        "generated binding should expose render_svg"
    );
    assert!(
        generated.contains("def render_ascii"),
        "generated binding should expose render_ascii"
    );
    assert!(
        generated.contains("def validate"),
        "generated binding should expose validate"
    );
    assert!(
        generated.contains("def analyze_document_json"),
        "generated binding should expose analyze_document_json"
    );
    assert!(
        generated.contains("def analyze_document_facts_json"),
        "generated binding should expose analyze_document_facts_json"
    );
    assert!(
        generated.contains("def supported_diagrams"),
        "generated binding should expose supported_diagrams"
    );
    assert!(
        generated.contains("def ascii_capabilities"),
        "generated binding should expose ascii_capabilities"
    );
    assert!(
        generated.contains("def presentation_catalog_json"),
        "generated binding should expose presentation_catalog_json"
    );
    assert!(
        !generated.contains("def supported_host_theme_presets"),
        "generated binding should not expose the removed host-theme compatibility method"
    );
    assert!(
        generated.contains("def diagram_family_capabilities"),
        "generated binding should expose diagram_family_capabilities"
    );
    assert!(
        generated.contains("def lint_rule_catalog"),
        "generated binding should expose lint_rule_catalog"
    );
    assert!(
        generated.contains("def reusable_engine_with_text_measurer"),
        "generated binding should expose reusable_engine_with_text_measurer"
    );
    assert!(
        !generated.contains("def set_text_measurer")
            && !generated.contains("def clear_text_measurer"),
        "generated bindings must keep host callbacks immutable after construction"
    );
    assert!(
        generated.contains("def binding_api_version"),
        "generated binding should expose its own binding API version"
    );
    assert!(
        generated.contains("def package_version"),
        "generated binding should expose package_version"
    );
    assert!(
        generated.contains("def runtime_catalog_json"),
        "generated binding should expose the atomic runtime catalog"
    );
    assert!(
        !generated.contains("def runtime_contract_json"),
        "generated binding must not expose the removed split runtime-contract endpoint"
    );
    assert!(
        !generated.contains("def runtime_capability_vocabulary_json"),
        "generated binding must not expose the removed split vocabulary endpoint"
    );
    assert!(
        generated.contains("class MermanError"),
        "generated binding should expose structured MermanError"
    );
    assert!(
        generated.contains("class MermanErrorKind")
            && generated.contains("class MermanResourceErrorDetails")
            && generated.contains("UNKNOWN_OPERATION")
            && !generated.contains("UNKNOWN_OUTPUT")
            && generated.contains("capability_id")
            && generated.contains("self.cause = cause")
            && generated.contains("self.resource")
            && generated.contains("self.kind"),
        "generated binding should expose machine-readable binding error fields"
    );
    assert!(
        generated.contains("class MermanValidationResult"),
        "generated binding should expose MermanValidationResult"
    );
    assert!(
        generated.contains("class MermanOperationRequest"),
        "generated binding should expose descriptor-owned operation requests"
    );
    let operation_request = generated
        .split_once("class MermanOperationRequest:")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("class _UniffiFfiConverterTypeMermanOperationRequest")
                .map(|(record, _)| record)
        })
        .expect("generated binding should contain the complete operation request record");
    assert!(
        operation_request.contains("operation_id")
            && operation_request.contains("self.operation_id = operation_id")
            && !operation_request.contains("output_id")
            && operation_request.contains("options_json")
            && operation_request.contains("self.options_json = options_json"),
        "generated operation requests should own operation_id and options_json"
    );
    assert!(
        generated.contains("class MermanOperationResult"),
        "generated binding should expose binary-safe operation results"
    );
    let operation_result = generated
        .split_once("class MermanOperationResult:")
        .and_then(|(_, suffix)| {
            suffix
                .split_once("class _UniffiFfiConverterTypeMermanOperationResult")
                .map(|(record, _)| record)
        })
        .expect("generated binding should contain the complete operation result record");
    assert!(
        operation_result.contains("operation_id")
            && operation_result.contains("self.operation_id = operation_id")
            && !operation_result.contains("output_id"),
        "generated operation results should expose operation_id without the old alias"
    );
    assert!(
        generated.contains("def execute"),
        "generated binding should expose the generic operation entry point"
    );
    assert!(
        !generated.contains("request: MermanOperationRequest,options_json"),
        "generated execute methods should not expose parallel request options"
    );
    assert!(
        !generated.contains("def render_svg(self, source: str) -> str:"),
        "generated reusable convenience methods should accept request-local options"
    );
    assert!(
        generated.contains("def render_png"),
        "generated binding should expose PNG bytes when the profile enables PNG"
    );
    assert!(
        generated.contains("def render_jpeg"),
        "generated binding should expose JPEG bytes when the profile enables JPEG"
    );
    assert!(
        generated.contains("def render_pdf"),
        "generated binding should expose PDF bytes when the profile enables PDF"
    );
}

#[test]
fn staged_python_package_imports_and_calls_rust_engine() {
    let Some(python) = python_executable() else {
        eprintln!("skipping Python package smoke because no Python executable was found");
        return;
    };

    let workspace_root = workspace_root();
    let cdylib = build_cdylib(&workspace_root);
    let package_dir = tempfile::tempdir().expect("create Python package smoke tempdir");
    let module_dir = package_dir.path().join("src").join("merman");
    fs::create_dir_all(&module_dir).expect("create staged Python module directory");
    let package_source = workspace_root
        .join("platforms")
        .join("python")
        .join("merman")
        .join("src")
        .join("merman");
    fs::copy(
        package_source.join("__init__.py"),
        module_dir.join("__init__.py"),
    )
    .expect("copy canonical Python package shim");
    fs::copy(
        package_source.join("_text_measurement_protocol.py"),
        module_dir.join("_text_measurement_protocol.py"),
    )
    .expect("copy canonical Python text-measurement protocol");
    fs::copy(
        package_source.join("_runtime_catalog.py"),
        module_dir.join("_runtime_catalog.py"),
    )
    .expect("copy canonical Python runtime catalog helper");
    fs::copy(
        package_source.join("_resource_options.py"),
        module_dir.join("_resource_options.py"),
    )
    .expect("copy canonical Python resource options helper");

    generate_python_bindings(&cdylib, &module_dir);
    copy_cdylib_next_to_generated_module(&cdylib, &module_dir);

    let smoke_script = workspace_root
        .join("platforms")
        .join("python")
        .join("merman")
        .join("examples")
        .join("smoke.py");

    let output = Command::new(python)
        .env("PYTHONPATH", package_dir.path().join("src"))
        .env("PYTHONUTF8", "1")
        .arg(&smoke_script)
        .output()
        .expect("run Python package smoke");

    assert!(
        output.status.success(),
        "Python package smoke failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let runtime_catalog_test = workspace_root
        .join("platforms")
        .join("python")
        .join("merman")
        .join("tests")
        .join("test_runtime_catalog.py");
    let output = Command::new(python)
        .env("PYTHONPATH", package_dir.path().join("src"))
        .env("PYTHONUTF8", "1")
        .arg(&runtime_catalog_test)
        .output()
        .expect("run Python runtime contract tests");
    assert!(
        output.status.success(),
        "Python runtime catalog tests failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert_python_native_runtime_catalog(
        python,
        package_dir.path().join("src"),
        workspace_root.join("capabilities/artifact-profiles-v1.json"),
    );
}

fn assert_python_native_runtime_catalog(
    python: &str,
    python_path: PathBuf,
    artifact_profiles: PathBuf,
) {
    const SCRIPT: &str = r#"
import json
import os

import merman

with open(os.environ["MERMAN_ARTIFACT_PROFILES"], encoding="utf-8") as source:
    profiles = json.load(source)["profiles"]
catalog = json.loads(merman.MermanEngine().runtime_catalog_json())
capabilities = catalog["capabilities"]

profile_id = "python-uniffi-native"
profile = next(profile for profile in profiles if profile["id"] == profile_id)
expected = profile["expected"]
assert capabilities["capability_ids"] == expected["capabilities"], (
    profile_id,
    capabilities["capability_ids"],
    expected["capabilities"],
)
assert capabilities["capability_ids"] == expected["runtime_ids"], (
    profile_id,
    capabilities["capability_ids"],
    expected["runtime_ids"],
)
assert capabilities["output_ids"] == expected["outputs"], (
    profile_id,
    capabilities["output_ids"],
    expected["outputs"],
)
"#;

    let output = Command::new(python)
        .env("PYTHONPATH", python_path)
        .env("PYTHONUTF8", "1")
        .env("MERMAN_ARTIFACT_PROFILES", artifact_profiles)
        .args(["-c", SCRIPT])
        .output()
        .expect("read the real UniFFI runtime catalog through generated Python");
    assert!(
        output.status.success(),
        "UniFFI runtime catalog drifted from python-uniffi-native\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn generate_python_bindings(cdylib: &Path, out_dir: &Path) {
    uniffi::generate(uniffi::GenerateOptions {
        languages: vec![uniffi::TargetLanguage::Python],
        source: utf8_path(cdylib).into(),
        out_dir: utf8_path(out_dir).into(),
        config_override: None,
        format: false,
        crate_filter: Some("merman_uniffi".to_string()),
        metadata_no_deps: false,
    })
    .expect("generate Python bindings from merman-uniffi cdylib metadata");
}

fn copy_cdylib_next_to_generated_module(cdylib: &Path, module_dir: &Path) {
    let file_name = cdylib
        .file_name()
        .unwrap_or_else(|| panic!("cdylib path has no file name: {}", cdylib.display()));
    fs::copy(cdylib, module_dir.join(file_name)).expect("copy cdylib into staged Python package");
}

fn python_executable() -> Option<&'static str> {
    ["python3", "python", "py"].into_iter().find(|candidate| {
        match Command::new(candidate).arg("--version").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    })
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("merman-uniffi should live under workspace/crates")
        .to_path_buf()
}

fn build_cdylib(workspace_root: &Path) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut features = artifact_profile_features(workspace_root, "python-uniffi-native");
    features.push("bindgen-smoke".to_string());
    let features = features.join(",");
    let status = Command::new(&cargo)
        .current_dir(workspace_root)
        .args([
            "build",
            "-p",
            "merman-uniffi",
            "--no-default-features",
            "--features",
            &features,
        ])
        .status()
        .expect("run cargo build for merman-uniffi cdylib");
    assert!(status.success(), "cargo build -p merman-uniffi failed");

    let target_dir = cargo_target_dir(workspace_root);
    let cdylib = target_dir.join("debug").join(cdylib_filename());
    assert!(
        cdylib.exists(),
        "expected merman-uniffi cdylib at {}",
        cdylib.display()
    );
    cdylib
}

fn artifact_profile_features(workspace_root: &Path, profile_id: &str) -> Vec<String> {
    let descriptor = fs::read_to_string(
        workspace_root
            .join("capabilities")
            .join("artifact-profiles-v1.json"),
    )
    .expect("read artifact profile descriptor");
    let descriptor: Value =
        serde_json::from_str(&descriptor).expect("parse artifact profile descriptor");
    descriptor["profiles"]
        .as_array()
        .expect("artifact profiles must be an array")
        .iter()
        .find(|profile| profile["id"] == profile_id)
        .unwrap_or_else(|| panic!("missing {profile_id} artifact profile"))["cargo"]["features"]
        .as_array()
        .expect("artifact profile Cargo features must be an array")
        .iter()
        .map(|feature| {
            feature
                .as_str()
                .expect("artifact profile feature must be a string")
                .to_string()
        })
        .collect()
}

fn cargo_target_dir(workspace_root: &Path) -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .current_dir(workspace_root)
        .args(["metadata", "--format-version=1", "--no-deps"])
        .output()
        .expect("run cargo metadata");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: Value =
        serde_json::from_slice(&output.stdout).expect("parse cargo metadata JSON");
    let target_directory = metadata
        .get("target_directory")
        .and_then(Value::as_str)
        .expect("cargo metadata target_directory");
    PathBuf::from(target_directory)
}

fn cdylib_filename() -> &'static str {
    if cfg!(windows) {
        "merman_uniffi.dll"
    } else if cfg!(target_os = "macos") {
        "libmerman_uniffi.dylib"
    } else {
        "libmerman_uniffi.so"
    }
}

fn generated_files_with_extension(dir: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_with_extension(dir, extension, &mut files);
    files.sort();
    files
}

fn collect_files_with_extension(dir: &Path, extension: &str, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read generated binding directory") {
        let entry = entry.expect("read generated binding entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_with_extension(&path, extension, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path);
        }
    }
}

fn utf8_path(path: &Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("path is not valid UTF-8: {}", path.display()))
        .to_string()
}

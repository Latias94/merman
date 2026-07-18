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
        generated.contains("def supported_host_theme_presets"),
        "generated binding should expose supported_host_theme_presets"
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
        generated.contains("def set_text_measurer"),
        "generated binding should expose set_text_measurer"
    );
    assert!(
        generated.contains("def clear_text_measurer"),
        "generated binding should expose clear_text_measurer"
    );
    assert!(
        generated.contains("def abi_version"),
        "generated binding should expose abi_version"
    );
    assert!(
        generated.contains("def package_version"),
        "generated binding should expose package_version"
    );
    assert!(
        generated.contains("class MermanError"),
        "generated binding should expose structured MermanError"
    );
    assert!(
        generated.contains("class MermanValidationResult"),
        "generated binding should expose MermanValidationResult"
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
    fs::write(module_dir.join("__init__.py"), PYTHON_PACKAGE_INIT)
        .expect("write staged Python package shim");
    fs::write(
        module_dir.join("_abi.py"),
        merman_uniffi::MERMAN_UNIFFI_PYTHON_ABI_MODULE,
    )
    .expect("write staged Python ABI contract");

    generate_python_bindings(&cdylib, &module_dir);
    copy_cdylib_next_to_generated_module(&cdylib, &module_dir);

    let smoke_script = package_dir.path().join("python_package_smoke.py");
    fs::write(&smoke_script, PYTHON_PACKAGE_SMOKE).expect("write Python package smoke script");

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
}

const PYTHON_PACKAGE_INIT: &str = r#"
from ._abi import (
    ABI_VERSION,
    TEXT_MEASUREMENT_OPERATIONS,
    TEXT_MEASUREMENT_RESULT_KINDS,
    AbiVersionMismatch,
    require_abi_version,
)
from .merman_uniffi import (
    MermanAsciiCapability,
    MermanAsciiCapabilityEvidence,
    MermanDiagramFamilyCapability,
    MermanEngine,
    MermanError,
    MermanLintRuleCatalogEntry,
    MermanReusableEngine,
    MermanTextDirection,
    MermanTextMeasureRequest,
    MermanTextMeasureResult,
    MermanTextMeasurementOperation,
    MermanTextMeasurementPhase,
    MermanTextMeasurementResultKind,
    MermanTextMeasurer,
    MermanTextWhiteSpace,
    MermanTextWrapMode,
    MermanValidationResult,
)

__all__ = [
    "ABI_VERSION",
    "AbiVersionMismatch",
    "TEXT_MEASUREMENT_OPERATIONS",
    "TEXT_MEASUREMENT_RESULT_KINDS",
    "MermanAsciiCapability",
    "MermanAsciiCapabilityEvidence",
    "MermanDiagramFamilyCapability",
    "MermanEngine",
    "MermanError",
    "MermanLintRuleCatalogEntry",
    "MermanReusableEngine",
    "MermanTextDirection",
    "MermanTextMeasureRequest",
    "MermanTextMeasureResult",
    "MermanTextMeasurementOperation",
    "MermanTextMeasurementPhase",
    "MermanTextMeasurementResultKind",
    "MermanTextMeasurer",
    "MermanTextWhiteSpace",
    "MermanTextWrapMode",
    "MermanValidationResult",
    "require_abi_version",
]
"#;

const PYTHON_PACKAGE_SMOKE: &str = r#"
import json
from dataclasses import dataclass

import merman

for exported_name in (
    "MermanTextMeasurementOperation",
    "MermanTextMeasurementPhase",
    "MermanTextMeasurementResultKind",
):
    assert hasattr(merman, exported_name), exported_name
    assert exported_name in merman.__all__, exported_name

operation_type = merman.MermanTextMeasurementOperation
assert {operation.value for operation in operation_type} == {
    entry[0] for entry in merman.TEXT_MEASUREMENT_OPERATIONS
}
assert {kind.value for kind in merman.MermanTextMeasurementResultKind} == {
    entry[0] for entry in merman.TEXT_MEASUREMENT_RESULT_KINDS
}

engine = merman.MermanEngine()
merman.require_abi_version(engine.abi_version())
try:
    merman.require_abi_version(merman.ABI_VERSION + 1)
except merman.AbiVersionMismatch as error:
    assert error.expected == merman.ABI_VERSION
    assert error.actual == merman.ABI_VERSION + 1
else:
    raise AssertionError("expected mismatched ABI to be rejected")
assert engine.package_version()
source = "---\ntitle: Host measurement phases\n---\nflowchart TD\nA[Hello] --> B[World]"

svg = engine.render_svg(source, None)
assert "<svg" in svg
assert "Hello" in svg
assert "World" in svg

ascii_text = engine.render_ascii(source, None)
assert "Hello" in ascii_text
assert "World" in ascii_text

parsed = json.loads(engine.parse_json(source, None))
assert parsed["type"] == "flowchart-v2"

layout = json.loads(engine.layout_json(source, None))
assert "meta" in layout
assert "layout" in layout

document_source = '# Example\n\n```mermaid\n' + source + '\n```\n'
document_analysis = json.loads(
    engine.analyze_document_json(document_source, None, "file:///tmp/example.md")
)
assert document_analysis["source"]["kind"] == "markdown"
assert document_analysis["valid"]

document_facts = json.loads(
    engine.analyze_document_facts_json(document_source, None, "file:///tmp/example.md")
)
assert document_facts["source"]["kind"] == "markdown"
assert document_facts["diagrams"][0]["source_id"] == "mermaid-fence-1"

validation = engine.validate(source, None)
assert validation.valid
assert validation.code_name == "MERMAN_OK"

invalid = engine.validate("", None)
assert not invalid.valid
assert invalid.code_name == "MERMAN_NO_DIAGRAM"
assert "no Mermaid diagram" in invalid.error

assert "flowchart" in engine.supported_diagrams()
ascii_capabilities = engine.ascii_capabilities()
assert any(
    item.diagram_type == "sequence" and item.support_level == "full"
    for item in ascii_capabilities
)
assert any(
    item.diagram_type == "gantt"
    and item.support_level == "summary"
    and not item.summary_fallback
    for item in ascii_capabilities
)
assert any(
    item.diagram_type == "class"
    and item.support_level == "partial"
    and item.summary_fallback
    for item in ascii_capabilities
)
assert "default" in engine.supported_themes()
assert "one-dark" in engine.supported_host_theme_presets()
assert any(
    item.diagram_type == "flowchart"
    for item in engine.diagram_family_capabilities()
)
lint_rules = engine.lint_rule_catalog()
assert any(
    rule.id == "merman.authoring.flowchart.explicit_direction"
    and rule.origin == "merman_authoring"
    and "docs/adr/0072-lint-rule-governance.md" in rule.evidence
    for rule in lint_rules
)
assert hasattr(merman, "MermanLintRuleCatalogEntry")
assert lint_rules
assert all(
    isinstance(rule, merman.MermanLintRuleCatalogEntry)
    for rule in lint_rules
)
assert all(rule.configurable for rule in engine.configurable_lint_rule_catalog())

@dataclass
class Measurer(merman.MermanTextMeasurer):
    calls: int = 0
    phases: set = None
    operations: set = None

    def __post_init__(self):
        self.phases = set()
        self.operations = set()

    def measure(self, request):
        self.calls += 1
        self.phases.add(request.phase)
        self.operations.add(request.operation)
        operation = request.operation.name
        width = max(len(request.text) * 8.0, 1.0)
        height = max(request.line_height, 1.0)
        values = dict(
            result_kind=merman.MermanTextMeasurementResultKind.METRICS,
            width=0.0,
            height=0.0,
            length=0.0,
            line_count=0,
            bbox_left=None,
            bbox_right=None,
            raw_width=None,
        )
        if operation in {"MEASURE", "WRAPPED", "MERMAID_CALCULATE_TEXT_DIMENSIONS"}:
            values.update(width=width, height=height, line_count=1)
        elif operation in {
            "COMPUTED_LENGTH",
            "SIMPLE_B_BOX_WIDTH",
            "RAW_B_BOX_WIDTH",
            "BOUNDING_CLIENT_RECT_WIDTH",
            "TSPAN_B_BOX_WIDTH",
            "WRAP_PROBE_B_BOX_WIDTH",
            "CANVAS_MEASURE_TEXT_WIDTH",
        }:
            values.update(
                result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
                length=width,
            )
        elif operation in {
            "TSPAN_B_BOX_HEIGHT",
            "SIMPLE_B_BOX_HEIGHT",
            "RAW_B_BOX_HEIGHT",
        }:
            values.update(
                result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
                length=height,
            )
        elif operation in {
            "CREATE_TEXT_B_BOX_Y_OFFSET",
            "CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET",
        }:
            values.update(
                result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
                length=(
                    -2.0
                    if operation == "CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET"
                    else -1.0
                ),
            )
        elif operation in {
            "B_BOX_X",
            "B_BOX_X_WITH_ASCII_OVERHANG",
            "TITLE_B_BOX_X",
        }:
            values.update(
                result_kind=merman.MermanTextMeasurementResultKind.HORIZONTAL_EXTENTS,
                bbox_left=width / 2.0,
                bbox_right=width / 2.0,
            )
        elif operation == "WRAPPED_WITH_RAW_WIDTH":
            values.update(
                result_kind=merman.MermanTextMeasurementResultKind.WRAPPED_WITH_RAW_WIDTH,
                width=width,
                height=height,
                line_count=1,
                raw_width=width,
            )
        else:
            return None
        return merman.MermanTextMeasureResult(
            **values,
        )


middle_y_offset = Measurer().measure(
    type(
        "MiddleYOffsetRequest",
        (),
        {
            "operation": operation_type.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET,
            "phase": merman.MermanTextMeasurementPhase.SVG_B_BOX,
            "text": "middle",
            "line_height": 16.0,
        },
    )()
)
assert middle_y_offset.result_kind == merman.MermanTextMeasurementResultKind.LENGTH
assert middle_y_offset.length < 0.0

raw_bbox_height = Measurer().measure(
    type(
        "RawBBoxHeightRequest",
        (),
        {
            "operation": operation_type.RAW_B_BOX_HEIGHT,
            "phase": merman.MermanTextMeasurementPhase.SVG_B_BOX,
            "text": "raw-height",
            "line_height": 18.0,
        },
    )()
)
assert raw_bbox_height.result_kind == merman.MermanTextMeasurementResultKind.LENGTH
assert raw_bbox_height.length == 18.0


measurer = Measurer()
reusable = engine.reusable_engine_with_text_measurer(None, measurer)
assert "Hello" in reusable.render_svg(source)
assert measurer.calls > 0
phase_names = {phase.name for phase in measurer.phases}
assert {"WRAP", "SVG_B_BOX"}.issubset(phase_names), phase_names
operation_names = {operation.name for operation in measurer.operations}
assert "WRAPPED" in operation_names, operation_names

setter_measurer = Measurer()
reusable = engine.reusable_engine(None)
reusable_document_analysis = json.loads(
    reusable.analyze_document_json(document_source, "file:///tmp/example.md")
)
assert reusable_document_analysis["source"]["kind"] == "markdown"
reusable_document_facts = json.loads(
    reusable.analyze_document_facts_json(document_source, "file:///tmp/example.md")
)
assert reusable_document_facts["source"]["kind"] == "markdown"
reusable.set_text_measurer(setter_measurer)
assert "Hello" in reusable.render_svg(source)
calls_after_set = setter_measurer.calls
assert calls_after_set > 0
reusable.clear_text_measurer()
assert "Hello" in reusable.render_svg(source)
assert setter_measurer.calls == calls_after_set

try:
    engine.render_svg(source, "{")
except merman.MermanError.Binding as exc:
    assert exc.code == 3
    assert exc.code_name == "MERMAN_OPTIONS_JSON_ERROR"
    assert "invalid options_json" in exc.message
else:
    raise AssertionError("expected invalid options_json to raise MermanError.Binding")

print("python package smoke passed")
"#;

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
    let status = Command::new(&cargo)
        .current_dir(workspace_root)
        .args([
            "build",
            "-p",
            "merman-uniffi",
            "--features",
            "bindgen-smoke",
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

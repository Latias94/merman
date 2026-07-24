#!/usr/bin/env python3
"""Generate the merman UniFFI Python package and build a local wheel."""

from __future__ import annotations

import argparse
from email.parser import Parser
import os
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

from artifact_profile_recipe import (
    CargoArtifactRecipe,
    cargo_build_args,
    cargo_run_example_args,
    load_artifact_profile,
)


REPO_ROOT = Path(__file__).resolve().parents[1]


def production_cdylib_path(recipe: CargoArtifactRecipe) -> Path:
    library_stem = recipe.target_name.replace("-", "_")
    if sys.platform == "win32":
        filename = f"{library_stem}.dll"
    elif sys.platform == "darwin":
        filename = f"lib{library_stem}.dylib"
    else:
        filename = f"lib{library_stem}.so"
    return REPO_ROOT / "target" / recipe.cargo_profile / filename


def validate_python_native_recipe(recipe: CargoArtifactRecipe) -> None:
    expected_target_contract = ("cdylib", "rlib", "staticlib")
    if (
        recipe.profile_id != "python-uniffi-native"
        or recipe.package != "merman-uniffi"
        or recipe.manifest != "crates/merman-uniffi/Cargo.toml"
        or recipe.cargo_profile != "native-sdk"
        or recipe.default_features
        or recipe.target_name != "merman_uniffi"
        or recipe.target_kinds != expected_target_contract
        or recipe.crate_types != expected_target_contract
        or recipe.build_target_kind != "host"
        or recipe.build_targets
    ):
        raise RuntimeError(
            "python-uniffi-native must remain the exact host native-sdk merman-uniffi "
            "complete native SDK cdylib recipe"
        )
    manifest = REPO_ROOT / recipe.manifest
    if not manifest.is_file():
        raise RuntimeError(f"python-uniffi-native manifest does not exist: {manifest}")


def python_generator_args(
    recipe: CargoArtifactRecipe,
    cdylib: Path,
    package_dir: Path,
) -> list[str]:
    return cargo_run_example_args(
        recipe,
        "generate_python_package",
        locked=True,
        extra_features=("bindgen-smoke",),
        example_args=(
            "--cdylib",
            str(cdylib),
            "--package-dir",
            str(package_dir),
        ),
    )


def python_generator_environment() -> dict[str, str]:
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(REPO_ROOT / "target" / "python-uniffi-bindgen")
    return environment

WHEEL_SMOKE = """
import merman


def measurement_result(operation, width, height):
    operation_type = merman.MermanTextMeasurementOperation
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
    if operation in {
        operation_type.MEASURE,
        operation_type.WRAPPED,
        operation_type.MERMAID_CALCULATE_TEXT_DIMENSIONS,
    }:
        values.update(width=width, height=height, line_count=1)
    elif operation in {
        operation_type.COMPUTED_LENGTH,
        operation_type.SIMPLE_B_BOX_WIDTH,
        operation_type.RAW_B_BOX_WIDTH,
        operation_type.BOUNDING_CLIENT_RECT_WIDTH,
        operation_type.TSPAN_B_BOX_WIDTH,
        operation_type.WRAP_PROBE_B_BOX_WIDTH,
        operation_type.CANVAS_MEASURE_TEXT_WIDTH,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=width,
        )
    elif operation in {
        operation_type.TSPAN_B_BOX_HEIGHT,
        operation_type.SIMPLE_B_BOX_HEIGHT,
        operation_type.RAW_B_BOX_HEIGHT,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=height,
        )
    elif operation in {
        operation_type.CREATE_TEXT_B_BOX_Y_OFFSET,
        operation_type.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.LENGTH,
            length=(
                -2.0
                if operation == operation_type.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET
                else -1.0
            ),
        )
    elif operation in {
        operation_type.B_BOX_X,
        operation_type.B_BOX_X_WITH_ASCII_OVERHANG,
        operation_type.TITLE_B_BOX_X,
    }:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.HORIZONTAL_EXTENTS,
            bbox_left=width / 2.0,
            bbox_right=width / 2.0,
        )
    elif operation == operation_type.WRAPPED_WITH_RAW_WIDTH:
        values.update(
            result_kind=merman.MermanTextMeasurementResultKind.WRAPPED_WITH_RAW_WIDTH,
            width=width,
            height=height,
            line_count=1,
            raw_width=width,
        )
    else:
        return None
    return merman.MermanTextMeasureResult(**values)


class Measurer(merman.MermanTextMeasurer):
    def __init__(self):
        self.calls = 0

    def measure(self, request):
        self.calls += 1
        return measurement_result(
            request.operation,
            max(len(request.text) * 8.0, 1.0),
            max(request.line_height, 1.0),
        )


operation_type = merman.MermanTextMeasurementOperation
expected_operation_codes = {entry[0] for entry in merman.TEXT_MEASUREMENT_OPERATIONS}
expected_result_kind_codes = {entry[0] for entry in merman.TEXT_MEASUREMENT_RESULT_KINDS}
assert {operation.value for operation in operation_type} == expected_operation_codes
assert {
    kind.value for kind in merman.MermanTextMeasurementResultKind
} == expected_result_kind_codes
dimensions = measurement_result(
    merman.MermanTextMeasurementOperation.MERMAID_CALCULATE_TEXT_DIMENSIONS,
    42.0,
    24.0,
)
assert dimensions.result_kind == merman.MermanTextMeasurementResultKind.METRICS
canvas_width = measurement_result(
    merman.MermanTextMeasurementOperation.CANVAS_MEASURE_TEXT_WIDTH,
    42.0,
    24.0,
)
assert canvas_width.result_kind == merman.MermanTextMeasurementResultKind.LENGTH
raw_bbox_height = measurement_result(
    merman.MermanTextMeasurementOperation.RAW_B_BOX_HEIGHT,
    42.0,
    24.0,
)
assert raw_bbox_height.result_kind == merman.MermanTextMeasurementResultKind.LENGTH
assert raw_bbox_height.length == 24.0
y_offset = measurement_result(
    merman.MermanTextMeasurementOperation.CREATE_TEXT_B_BOX_Y_OFFSET,
    42.0,
    24.0,
)
assert y_offset.length < 0.0
middle_y_offset = measurement_result(
    merman.MermanTextMeasurementOperation.CREATE_TEXT_MIDDLE_B_BOX_Y_OFFSET,
    42.0,
    24.0,
)
assert middle_y_offset.result_kind == merman.MermanTextMeasurementResultKind.LENGTH
assert middle_y_offset.length < 0.0

engine = merman.MermanEngine()
assert engine.binding_api_version() == 3
assert engine.package_version()
catalog = merman.get_runtime_catalog(engine)
capabilities = catalog["capabilities"]
assert catalog["schema_version"] == 1
assert catalog["transport_api_version"] == engine.binding_api_version()
assert catalog["package_version"] == engine.package_version()
assert capabilities["capability_ids"] == sorted(capabilities["capability_ids"])
assert capabilities["output_ids"] == sorted(capabilities["output_ids"])
assert capabilities["operation_ids"] == sorted(capabilities["operation_ids"])
assert not hasattr(merman, "get_runtime_contract")
assert not hasattr(merman, "get_runtime_capability_vocabulary")
required_capabilities = {
    "analysis",
    "ascii",
    "jpeg",
    "layout-cytoscape",
    "layout-elk",
    "math",
    "pdf",
    "png",
    "svg",
}
assert required_capabilities.issubset(
    set(capabilities["capability_ids"])
)
assert {"ascii", "jpeg", "pdf", "png", "svg"}.issubset(
    set(capabilities["output_ids"])
)
assert set(capabilities["output_ids"]).issubset(capabilities["operation_ids"])
source = "flowchart TD\\nA[Hello] --> B[World]"
try:
    engine.execute(
        merman.MermanOperationRequest(
            operation_id="not-an-operation",
            source=source,
            uri=None,
            options_json=None,
        )
    )
except merman.MermanError.Binding as error:
    assert error.kind == merman.MermanErrorKind.UNKNOWN_OPERATION
    assert error.capability_id is None
else:
    raise AssertionError("unknown operation did not preserve its typed binding error")
assert "Hello" in engine.render_svg(source, None)
assert "Hello" in engine.render_ascii(source, None)
assert engine.render_png(source, None).startswith(b"\\x89PNG\\r\\n\\x1a\\n")
assert engine.render_jpeg(source, None).startswith(b"\\xff\\xd8\\xff")
assert engine.render_pdf(source, None).startswith(b"%PDF-")
assert "flowchart-v2" in engine.parse_json(source, None)
assert "layout" in engine.layout_json(source, None)
assert engine.validate(source, None).valid
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
assert any(item.diagram_type == "flowchart" for item in engine.diagram_family_capabilities())
assert hasattr(merman, "MermanLintRuleCatalogEntry")
lint_rules = engine.lint_rule_catalog()
assert lint_rules
assert all(isinstance(rule, merman.MermanLintRuleCatalogEntry) for rule in lint_rules)
assert any(
    rule.id == "merman.authoring.flowchart.explicit_direction"
    and rule.origin == "merman_authoring"
    for rule in lint_rules
)
configurable_rules = engine.configurable_lint_rule_catalog()
assert configurable_rules
assert all(
    isinstance(rule, merman.MermanLintRuleCatalogEntry) for rule in configurable_rules
)
assert any(
    rule.id == "merman.authoring.flowchart.explicit_direction"
    and rule.configurable
    for rule in configurable_rules
)
assert all(rule.configurable for rule in configurable_rules)

measurer = Measurer()
reusable = engine.reusable_engine_with_text_measurer(None, measurer)
assert reusable.render_svg(source, None).startswith("<svg")
assert "Hello" in reusable.render_ascii(source, None)
assert "flowchart-v2" in reusable.parse_json(source, None)
assert "layout" in reusable.layout_json(source, None)
assert reusable.validate(source, None).valid
assert measurer.calls > 0

setter_measurer = Measurer()
reusable = engine.reusable_engine(None)
reusable.set_text_measurer(setter_measurer)
assert reusable.render_svg(source, None).startswith("<svg")
calls_after_set = setter_measurer.calls
assert calls_after_set > 0
reusable.clear_text_measurer()
assert reusable.render_svg(source, None).startswith("<svg")
assert setter_measurer.calls == calls_after_set


class FailingMeasurer(merman.MermanTextMeasurer):
    def __init__(self):
        self.calls = 0

    def measure(self, request):
        self.calls += 1
        raise RuntimeError("host measurer failed")


failing_measurer = FailingMeasurer()
failing = engine.reusable_engine_with_text_measurer(None, failing_measurer)
assert failing.render_svg(source, None).startswith("<svg")
assert failing_measurer.calls > 0
failing.set_text_measurer(Measurer())
assert failing.render_svg(source, None).startswith("<svg")
print("python wheel smoke passed")
"""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package-dir",
        default=str(REPO_ROOT / "platforms" / "python" / "merman"),
        help="Python package scaffold directory.",
    )
    parser.add_argument(
        "--wheel-dir",
        default=str(REPO_ROOT / "target" / "python-wheels"),
        help="Output directory for built wheels.",
    )
    parser.add_argument(
        "--python",
        default=sys.executable,
        help="Python executable used for pip, venv, and smoke checks.",
    )
    parser.add_argument(
        "--run-smoke",
        action="store_true",
        help="Install the newest wheel into a temporary venv and run an import/render smoke.",
    )
    return parser.parse_args()


def run(
    args: list[str],
    *,
    cwd: Path = REPO_ROOT,
    env: dict[str, str] | None = None,
) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, env=env, check=True)


def venv_python(venv_dir: Path) -> Path:
    windows_python = venv_dir / "Scripts" / "python.exe"
    if windows_python.exists():
        return windows_python
    unix_python = venv_dir / "bin" / "python"
    if unix_python.exists():
        return unix_python
    raise RuntimeError(f"Python executable not found in venv: {venv_dir}")


def newest_wheel(wheel_dir: Path) -> Path:
    wheels = sorted(
        wheel_dir.glob("merman-*.whl"),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    if not wheels:
        raise RuntimeError(f"No merman wheel found under {wheel_dir}")
    return wheels[0]


def remove_stale_wheels(wheel_dir: Path) -> None:
    for wheel in wheel_dir.glob("merman-*.whl"):
        wheel.unlink()


def remove_stale_package_build(package_dir: Path) -> None:
    build_dir = package_dir / "build"
    if build_dir.exists():
        shutil.rmtree(build_dir)


def require_platform_wheel(wheel: Path) -> None:
    if wheel.name.endswith("-py3-none-any.whl"):
        raise RuntimeError(
            f"expected a platform wheel with the bundled native library, got universal wheel: {wheel.name}"
        )


def require_native_platlib_layout(wheel: Path) -> None:
    native_suffixes = (".dll", ".dylib", ".so")
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        wheel_metadata_path = next(
            (name for name in names if name.endswith(".dist-info/WHEEL")), None
        )
        if wheel_metadata_path is None:
            raise RuntimeError(f"{wheel.name} does not contain WHEEL metadata")

        metadata = Parser().parsestr(archive.read(wheel_metadata_path).decode("utf-8"))
        if metadata.get("Root-Is-Purelib") != "false":
            raise RuntimeError(
                f"{wheel.name} must set Root-Is-Purelib: false for bundled native libraries"
            )

        native_members = [
            name for name in names if name.lower().endswith(native_suffixes)
        ]
        if not native_members:
            raise RuntimeError(f"{wheel.name} does not contain a bundled native library")

        purelib_native_members = [
            name for name in native_members if ".data/purelib/" in name
        ]
        if purelib_native_members:
            joined = ", ".join(purelib_native_members)
            raise RuntimeError(
                f"{wheel.name} stores native libraries under purelib: {joined}"
            )


def main() -> int:
    args = parse_args()
    package_dir = Path(args.package_dir).expanduser().resolve()
    wheel_dir = Path(args.wheel_dir).expanduser().resolve()

    recipe = load_artifact_profile("python-uniffi-native")
    validate_python_native_recipe(recipe)
    run(cargo_build_args(recipe, locked=True))
    cdylib = production_cdylib_path(recipe)
    if not cdylib.is_file():
        raise RuntimeError(f"expected production UniFFI library not found: {cdylib}")
    run(
        python_generator_args(recipe, cdylib, package_dir),
        env=python_generator_environment(),
    )

    remove_stale_package_build(package_dir)
    wheel_dir.mkdir(parents=True, exist_ok=True)
    remove_stale_wheels(wheel_dir)
    run(
        [
            args.python,
            "-m",
            "pip",
            "wheel",
            str(package_dir),
            "--no-deps",
            "--wheel-dir",
            str(wheel_dir),
        ]
    )
    wheel = newest_wheel(wheel_dir)
    require_platform_wheel(wheel)
    require_native_platlib_layout(wheel)

    if args.run_smoke:
        venv_dir = REPO_ROOT / "target" / "python-wheel-smoke"
        if venv_dir.exists():
            shutil.rmtree(venv_dir)
        run([args.python, "-m", "venv", str(venv_dir)])
        python = venv_python(venv_dir)
        run([str(python), "-m", "pip", "install", "--no-deps", str(wheel)])
        run(
            [
                str(python),
                "-c",
                WHEEL_SMOKE,
            ]
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Generate the merman UniFFI Python package and build a local wheel."""

from __future__ import annotations

import argparse
from contextlib import contextmanager
from email.parser import Parser
import json
import os
import shutil
import subprocess
import sys
import tempfile
import zipfile
from collections.abc import Iterator
from pathlib import Path

from artifact_profile_recipe import (
    DEFAULT_DESCRIPTOR,
    CargoArtifactRecipe,
    cargo_build_args,
    cargo_run_example_args,
    load_artifact_profile,
    rustc_host_target,
)
from capability_surface_contract import (
    canonical_capability_surface,
    capability_surface_digest,
    validate_capability_authority,
)
from python_wheel_licenses import (
    install_target_report,
    verify_wheel_license_report,
)
from strict_json import StrictJsonContract


REPO_ROOT = Path(__file__).resolve().parents[1]
CAPABILITY_DESCRIPTOR = REPO_ROOT / "capabilities" / "feature-surface-v1.json"
WHEEL_JSON = StrictJsonContract(
    error_factory=RuntimeError,
    read_error_prefix="cannot read Python wheel contract",
)
SEMANTIC_OPERATION_FIXTURES = (
    REPO_ROOT / "fixtures" / "bindings" / "assets" / "semantic-operations-v1.json"
)
PYTHON_GENERATED_SUPPORT_FILES = (
    "src/merman/__init__.py",
    "src/merman/_binding_contract.py",
    "src/merman/_resource_options.py",
    "src/merman/_runtime_catalog.py",
    "src/merman/_text_measurement_protocol.py",
)
PYTHON_STAGING_IGNORE = shutil.ignore_patterns(
    "build",
    "dist",
    "*.egg-info",
    "__pycache__",
    "merman_uniffi.py",
    "*.dll",
    "*.dylib",
    "*.so",
)


def production_cdylib_path(recipe: CargoArtifactRecipe, target: str) -> Path:
    library_stem = recipe.target_name.replace("-", "_")
    if "windows" in target:
        filename = f"{library_stem}.dll"
    elif "apple" in target:
        filename = f"lib{library_stem}.dylib"
    else:
        filename = f"lib{library_stem}.so"
    return REPO_ROOT / "target" / target / recipe.cargo_profile / filename


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
        or recipe.build_target_kind != "target-set"
        or not recipe.build_targets
    ):
        raise RuntimeError(
            "python-uniffi-native must remain the exact target-set native-sdk "
            "merman-uniffi complete native SDK cdylib recipe"
        )
    manifest = REPO_ROOT / recipe.manifest
    if not manifest.is_file():
        raise RuntimeError(f"python-uniffi-native manifest does not exist: {manifest}")


def select_python_wheel_target(recipe: CargoArtifactRecipe) -> str:
    target = rustc_host_target()
    if target not in recipe.build_targets:
        raise RuntimeError(
            f"Python wheels are not published for Rust host target {target!r}; "
            f"supported targets: {', '.join(recipe.build_targets)}"
        )
    return target


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
import json
import os

import merman


def assert_shared_semantic_operation_fixtures(engine):
    fixture_path = os.environ["MERMAN_SEMANTIC_OPERATION_FIXTURES"]
    with open(fixture_path, encoding="utf-8") as handle:
        fixture_root = json.load(handle)
    assert set(fixture_root) == {"schema_version", "cases"}
    assert fixture_root["schema_version"] == 1
    assert fixture_root["cases"]

    for index, case in enumerate(fixture_root["cases"]):
        label = f"fixture {index} operation `{case['operation_id']}`"
        options = case.get("options")
        request = merman.MermanOperationRequest(
            operation_id=case["operation_id"],
            source=case["source"],
            uri=case.get("uri"),
            options_json=(
                json.dumps(options, separators=(",", ":"))
                if options is not None
                else None
            ),
        )
        try:
            result = engine.execute(request)
        except merman.MermanError.Binding as error:
            assert case.get("expected_media_type") is None, label
            assert case.get("expected_error_kind") == "generic", label
            assert error.kind == merman.MermanErrorKind.GENERIC, label
            assert error.capability_id is None, label
            assert error.message, label
            assert case["payload_invariants"] == ["error-message-nonempty"], label
            continue

        assert case.get("expected_error_kind") is None, label
        assert result.operation_id == case["operation_id"], label
        assert result.media_type == case["expected_media_type"], label
        for invariant in case["payload_invariants"]:
            if invariant == "nonempty":
                assert result.data, label
            elif invariant == "utf8":
                result.data.decode("utf-8")
            elif invariant == "json-object":
                assert isinstance(json.loads(result.data), dict), label
            elif invariant == "svg-root":
                assert result.data.lstrip().startswith(b"<svg"), label
            elif invariant == "metadata-operation-id":
                metadata = json.loads(result.metadata_json)
                assert metadata["operation_id"] == case["operation_id"], label
            else:
                raise AssertionError(f"{label} has unsupported invariant `{invariant}`")


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
assert capabilities["capability_ids"] == EXPECTED_CAPABILITY_IDS
assert capabilities["capability_ids"] == EXPECTED_RUNTIME_IDS
assert capabilities["output_ids"] == EXPECTED_OUTPUT_IDS
assert capabilities["operation_ids"] == EXPECTED_OPERATION_IDS
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
assert_shared_semantic_operation_fixtures(engine)
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
callback_engine = engine.reusable_engine_with_text_measurer(None, setter_measurer)
assert callback_engine.render_svg(source, None).startswith("<svg")
assert setter_measurer.calls > 0
plain_engine = engine.reusable_engine(None)
assert plain_engine.render_svg(source, None).startswith("<svg")
assert not hasattr(callback_engine, "set_text_measurer")
assert not hasattr(callback_engine, "clear_text_measurer")


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
replacement = engine.reusable_engine_with_text_measurer(None, Measurer())
assert replacement.render_svg(source, None).startswith("<svg")
print("python wheel smoke passed")
"""


def python_wheel_smoke_script(profile_id: str) -> str:
    descriptor = WHEEL_JSON.object(
        WHEEL_JSON.load(DEFAULT_DESCRIPTOR),
        "artifact profile descriptor",
    )
    capability_descriptor = WHEEL_JSON.object(
        WHEEL_JSON.load(CAPABILITY_DESCRIPTOR),
        "capability descriptor",
    )
    capability_descriptor = _validate_capability_authority(
        descriptor,
        capability_descriptor,
    )
    profiles = WHEEL_JSON.array(
        descriptor.get("profiles"),
        "artifact profile descriptor profiles",
    )
    matches = [
        profile
        for profile in profiles
        if isinstance(profile, dict) and profile.get("id") == profile_id
    ]
    if len(matches) != 1:
        raise RuntimeError(
            f"artifact profile {profile_id!r} must occur exactly once; found {len(matches)}"
        )
    expected = matches[0].get("expected")
    if not isinstance(expected, dict):
        raise RuntimeError(f"artifact profile {profile_id!r} has no expected contract")
    values: dict[str, list[str]] = {}
    for field in ("capabilities", "runtime_ids", "outputs"):
        value = expected.get(field)
        if (
            not isinstance(value, list)
            or any(not isinstance(item, str) or not item for item in value)
            or value != sorted(set(value))
        ):
            raise RuntimeError(
                f"artifact profile {profile_id!r} expected.{field} must be sorted unique strings"
            )
        values[field] = value
    expected_capabilities = set(values["capabilities"])
    expected_operations = _native_binding_operation_ids(
        capability_descriptor,
        expected_capabilities,
    )
    return (
        f"EXPECTED_CAPABILITY_IDS = {values['capabilities']!r}\n"
        f"EXPECTED_RUNTIME_IDS = {values['runtime_ids']!r}\n"
        f"EXPECTED_OUTPUT_IDS = {values['outputs']!r}\n"
        f"EXPECTED_OPERATION_IDS = {expected_operations!r}\n"
        + WHEEL_SMOKE
    )


def _validate_capability_authority(
    profiles_descriptor: dict[str, object],
    capability_descriptor: dict[str, object],
) -> dict[str, object]:
    try:
        descriptor_root = DEFAULT_DESCRIPTOR.parent.parent
        expected_path = CAPABILITY_DESCRIPTOR.relative_to(descriptor_root).as_posix()
    except ValueError as error:
        raise RuntimeError(
            "capability descriptor path must share the artifact profile descriptor root"
        ) from error
    return validate_capability_authority(
        profiles_descriptor,
        capability_descriptor,
        expected_path=expected_path,
        error_factory=RuntimeError,
        profiles_context="artifact profile",
        capability_context="capability descriptor",
        expected_schema_version=1,
        require_sorted_compiled_prerequisites=True,
    )


def _canonical_capability_descriptor(
    descriptor: object,
) -> dict[str, object]:
    return canonical_capability_surface(
        descriptor,
        error_factory=RuntimeError,
        context="capability descriptor",
        expected_schema_version=1,
        require_sorted_compiled_prerequisites=True,
    )


def _capability_descriptor_digest(descriptor: dict[str, object]) -> str:
    return capability_surface_digest(descriptor)


def _native_binding_operation_ids(
    canonical_descriptor: dict[str, object],
    expected_capabilities: set[str],
) -> list[str]:
    selected: list[str] = []
    for operation in canonical_descriptor["binding_operations"]:
        assert isinstance(operation, dict)
        operation_id = operation["id"]
        capability = operation["capability"]
        targets = operation["targets"]
        if "native" in targets and (
            capability is None or capability in expected_capabilities
        ):
            assert isinstance(operation_id, str)
            selected.append(operation_id)

    return sorted(selected)


def wheel_smoke_environment() -> dict[str, str]:
    if not SEMANTIC_OPERATION_FIXTURES.is_file():
        raise RuntimeError(
            f"semantic operation fixtures are missing: {SEMANTIC_OPERATION_FIXTURES}"
        )
    environment = os.environ.copy()
    environment["MERMAN_SEMANTIC_OPERATION_FIXTURES"] = str(
        SEMANTIC_OPERATION_FIXTURES
    )
    return environment


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
    display_args = ("<inline-script>" if "\n" in arg else arg for arg in args)
    print("+", " ".join(display_args))
    subprocess.run(args, cwd=cwd, env=env, check=True)


def require_tracked_python_support_files(package_dir: Path) -> None:
    for relative in PYTHON_GENERATED_SUPPORT_FILES:
        source = package_dir / relative
        if not source.is_file():
            raise RuntimeError(f"generated Python support file is missing: {source}")
        try:
            repository_path = source.resolve().relative_to(REPO_ROOT.resolve()).as_posix()
        except ValueError as exc:
            raise RuntimeError(
                f"Python package source must be inside the repository: {package_dir}"
            ) from exc
        run(["git", "ls-files", "--error-unmatch", "--", repository_path])


@contextmanager
def staged_python_package(package_dir: Path) -> Iterator[Path]:
    with tempfile.TemporaryDirectory(prefix="merman-python-wheel-") as temp_dir:
        staged = Path(temp_dir) / package_dir.name
        shutil.copytree(package_dir, staged, ignore=PYTHON_STAGING_IGNORE)
        yield staged


def verify_staged_python_support_files(package_dir: Path, staged: Path) -> None:
    for relative in PYTHON_GENERATED_SUPPORT_FILES:
        source = package_dir / relative
        generated = staged / relative
        if not generated.is_file():
            raise RuntimeError(
                f"Python generator did not produce required support file: {relative}"
            )
        if source.read_bytes() != generated.read_bytes():
            raise RuntimeError(
                "stale generated Python support file: "
                f"{relative}; regenerate and commit the source projection"
            )


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
    package_source = Path(args.package_dir).expanduser().resolve()
    wheel_dir = Path(args.wheel_dir).expanduser().resolve()

    recipe = load_artifact_profile("python-uniffi-native")
    validate_python_native_recipe(recipe)
    target = select_python_wheel_target(recipe)
    require_tracked_python_support_files(package_source)
    run(cargo_build_args(recipe, locked=True, target=target))
    cdylib = production_cdylib_path(recipe, target)
    if not cdylib.is_file():
        raise RuntimeError(f"expected production UniFFI library not found: {cdylib}")
    with staged_python_package(package_source) as package_dir:
        run(
            python_generator_args(recipe, cdylib, package_dir),
            env=python_generator_environment(),
        )
        verify_staged_python_support_files(package_source, package_dir)
        install_target_report(REPO_ROOT, package_dir, target)

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
        verify_wheel_license_report(
            wheel,
            root=REPO_ROOT,
            expected_target=target,
        )

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
                python_wheel_smoke_script(recipe.profile_id),
            ],
            env=wheel_smoke_environment(),
        )
        example = package_source / "examples" / "smoke.py"
        if not example.is_file():
            raise RuntimeError(f"Python wheel smoke example is missing: {example}")
        run([str(python), str(example)])

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

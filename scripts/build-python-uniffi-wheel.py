#!/usr/bin/env python3
"""Generate the merman UniFFI Python package and build a local wheel."""

from __future__ import annotations

import argparse
from email.parser import Parser
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]

WHEEL_SMOKE = """
import merman


class Measurer(merman.MermanTextMeasurer):
    def __init__(self):
        self.calls = 0

    def measure(self, request):
        self.calls += 1
        return merman.MermanTextMeasureResult(
            width=max(len(request.text) * 8.0, 1.0),
            height=max(request.line_height, 1.0),
            line_count=1,
        )


engine = merman.MermanEngine()
assert engine.abi_version() == 2
assert engine.package_version()
source = "flowchart TD\\nA[Hello] --> B[World]"
assert "Hello" in engine.render_svg(source, None)
assert "Hello" in engine.render_ascii(source, None)
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
assert all(rule.configurable for rule in engine.configurable_lint_rule_catalog())

measurer = Measurer()
reusable = engine.reusable_engine_with_text_measurer(None, measurer)
assert reusable.render_svg(source).startswith("<svg")
assert "Hello" in reusable.render_ascii(source)
assert "flowchart-v2" in reusable.parse_json(source)
assert "layout" in reusable.layout_json(source)
assert reusable.validate(source).valid
assert measurer.calls > 0

setter_measurer = Measurer()
reusable = engine.reusable_engine(None)
reusable.set_text_measurer(setter_measurer)
assert reusable.render_svg(source).startswith("<svg")
calls_after_set = setter_measurer.calls
assert calls_after_set > 0
reusable.clear_text_measurer()
assert reusable.render_svg(source).startswith("<svg")
assert setter_measurer.calls == calls_after_set


class FailingMeasurer(merman.MermanTextMeasurer):
    def measure(self, request):
        raise RuntimeError("host measurer failed")


failing = engine.reusable_engine_with_text_measurer(None, FailingMeasurer())
try:
    failing.render_svg(source)
except merman.MermanError.Binding as error:
    assert "host measurer failed" in error.message
else:
    raise AssertionError("expected host text measurer callback failure")
failing.set_text_measurer(Measurer())
assert failing.render_svg(source).startswith("<svg")
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


def run(args: list[str], *, cwd: Path = REPO_ROOT) -> None:
    print("+", " ".join(args))
    subprocess.run(args, cwd=cwd, check=True)


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

    run(["cargo", "build", "-p", "merman-uniffi", "--features", "bindgen-smoke"])
    run(
        [
            "cargo",
            "run",
            "-p",
            "merman-uniffi",
            "--features",
            "bindgen-smoke",
            "--example",
            "generate_python_package",
            "--",
            "--package-dir",
            str(package_dir),
        ]
    )

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

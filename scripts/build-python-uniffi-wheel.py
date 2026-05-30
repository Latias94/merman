#!/usr/bin/env python3
"""Generate the merman UniFFI Python package and build a local wheel."""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package-dir",
        default=str(REPO_ROOT / "bindings" / "python" / "merman"),
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
    run([args.python, "-m", "pip", "wheel", str(package_dir), "--no-deps", "--wheel-dir", str(wheel_dir)])

    if args.run_smoke:
        wheel = newest_wheel(wheel_dir)
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
                "import merman; e = merman.MermanEngine(); assert e.render_svg('flowchart TD\\nA[Hello]', None).startswith('<svg')",
            ]
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

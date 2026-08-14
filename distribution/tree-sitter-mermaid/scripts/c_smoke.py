"""Compile and run the generated C language entry point without a generator."""

from __future__ import annotations

import json
import os
import shlex
import shutil
import subprocess
import tempfile
from pathlib import Path


TREE_SITTER_RUNTIME_VERSION = "0.26.12"


def runtime_directory(package: Path) -> Path:
    override = os.environ.get("TREE_SITTER_RUNTIME_DIR")
    if override:
        runtime = Path(override).resolve()
    else:
        cargo = os.environ.get("CARGO", "cargo")
        result = subprocess.run(
            [cargo, "metadata", "--locked", "--format-version", "1"],
            cwd=package.parents[1],
            check=True,
            capture_output=True,
            text=True,
        )
        packages = json.loads(result.stdout)["packages"]
        matches = [
            Path(item["manifest_path"]).parent
            for item in packages
            if item["name"] == "tree-sitter"
            and item["version"] == TREE_SITTER_RUNTIME_VERSION
        ]
        if len(matches) != 1:
            raise SystemExit(
                f"expected one tree-sitter {TREE_SITTER_RUNTIME_VERSION} runtime, "
                f"found {len(matches)}"
            )
        runtime = matches[0]
    if not (runtime / "src/lib.c").is_file() or not (
        runtime / "include/tree_sitter/api.h"
    ).is_file():
        raise SystemExit(f"invalid Tree-sitter runtime directory: {runtime}")
    return runtime


def compiler_command(
    compiler: list[str], runtime: Path, output: Path
) -> list[str]:
    executable = Path(compiler[0]).name.lower().removesuffix(".exe")
    sources = [
        "tests/c_smoke.c",
        "src/parser.c",
        "src/scanner.c",
        str(runtime / "src/lib.c"),
    ]
    if executable in {"cl", "clang-cl"}:
        return [
            *compiler,
            "/nologo",
            "/std:c11",
            "/Isrc",
            "/Ibindings/c",
            f"/I{runtime / 'include'}",
            f"/I{runtime / 'src'}",
            *sources,
            f"/Fe:{output}",
        ]
    return [
        *compiler,
        "-std=c11",
        "-Isrc",
        "-Ibindings/c",
        f"-I{runtime / 'include'}",
        f"-I{runtime / 'src'}",
        *sources,
        "-o",
        str(output),
    ]


def find_compiler() -> list[str]:
    configured = os.environ.get("CC")
    specifications = (
        [configured]
        if configured
        else (["cl", "clang-cl", "cc"] if os.name == "nt" else ["cc", "clang", "gcc"])
    )
    for specification in specifications:
        compiler = shlex.split(specification, posix=os.name != "nt")
        if compiler and shutil.which(compiler[0]) is not None:
            return compiler
    expected = configured or ", ".join(specifications)
    raise SystemExit(f"C compiler not found: {expected}")


def main() -> int:
    package = Path(__file__).resolve().parents[1]
    compiler = find_compiler()
    runtime = runtime_directory(package)
    fixtures = json.loads(
        (package / "metadata/fixtures/family-roots.json").read_text(encoding="utf-8")
    )
    if len(fixtures) != 35:
        raise SystemExit(f"expected 35 public family fixtures, found {len(fixtures)}")
    receipt_id = json.loads(
        (package / "metadata/artifact-receipt.json").read_text(encoding="utf-8")
    )["receiptId"]

    output_name = (
        "tree-sitter-mermaid-c-smoke.exe"
        if os.name == "nt"
        else "tree-sitter-mermaid-c-smoke"
    )
    with tempfile.TemporaryDirectory(prefix="tree-sitter-mermaid-c-") as directory:
        output = Path(directory) / output_name
        command = compiler_command(compiler, runtime, output)
        subprocess.run(command, cwd=package, check=True)
        for fixture in fixtures:
            subprocess.run(
                [str(output), fixture["source"], fixture["root"], receipt_id],
                cwd=package,
                check=True,
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

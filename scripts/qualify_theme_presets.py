#!/usr/bin/env python3
"""Record or recheck scoped preset qualification from a clean candidate build.

The Rust runner owns qualification. This script binds its output to the source commit and actual
executable, and reexecutes it for freshness checks. Only a matched CLI receives an artifact-local
catalog projection; shared production catalogs remain unchanged.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

try:
    from .theme_preset_catalog_contract import validate_unqualified_catalog
    from .run_theme_acceptance import acceptance_environment
    from .release_process import CommandRunner, run_checked
except ImportError:
    from theme_preset_catalog_contract import validate_unqualified_catalog
    from run_theme_acceptance import acceptance_environment
    from release_process import CommandRunner, run_checked

ROOT = Path(__file__).resolve().parents[1]
QUALIFIED_PROFILE = "native-flowchart-state-sequence-system-fonts-v3"
QUALIFIED_ADMISSION = "host_dependent"
QUALIFIED_FAMILIES = {"flowchart", "state", "sequence"}
QUALIFIED_OUTPUTS = {"svg", "png"}

BUILD = [
    "cargo", "build", "--release", "--locked", "-p", "merman-theme-acceptance",
    "--no-default-features", "--features", "png,layout-cytoscape", "--example", "preset_qualification",
    "--jobs", "2", "--message-format=json-render-diagnostics",
]


def clean_revision(root: Path) -> str:
    status = subprocess.check_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], cwd=root, text=True,
    )
    if status.strip():
        raise RuntimeError("Qualification requires a clean checkout, including untracked files")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()


def sha256_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def qualify_cli(
    binary: Path, qualification: dict, *, runner: CommandRunner = subprocess.run,
) -> dict:
    """Bind a real CLI to fresh Rust-qualified bytes, without reimplementing their checks."""
    binary = binary.resolve(strict=True)
    digest = sha256_file(binary)
    config = {"htmlLabels": False}
    if not qualification.get("presets"):
        raise RuntimeError("CLI qualification requires actual target observations")
    matched = 0
    with tempfile.TemporaryDirectory(prefix="merman-preset-cli-") as temporary:
        directory = Path(temporary)
        config_path = directory / "config.json"
        config_path.write_text(json.dumps(config), encoding="utf-8")
        for preset in qualification["presets"]:
            for cell in preset["cells"]:
                source = cell["source"].encode("utf-8")
                if hashlib.sha256(source).hexdigest() != cell["source_digest"]:
                    raise RuntimeError("CLI qualification source digest differs from the Rust scenario")
                command = [
                    str(binary), "render", "--theme-preset", preset["preset"],
                    "--format", cell["output"], "--config-file", str(config_path),
                ]
                if cell["output"] == "svg":
                    command.extend(["--svg-pipeline", "resvg-safe"])
                elif cell["output"] == "png":
                    command.extend(["--scale", str(cell["png_scale"])])
                else:
                    raise RuntimeError(f"Undeclared CLI qualification target: {cell['output']}")
                result = run_checked(command + ["-"], stdin=source, cwd=directory, runner=runner)
                if hashlib.sha256(result.stdout).hexdigest() != cell["artifact_digest"]:
                    raise RuntimeError(
                        f"CLI output differs from qualified bytes: "
                        f"{preset['preset']}/{cell['source_id']}/{cell['output']}"
                    )
                matched += 1
    if matched == 0:
        raise RuntimeError("CLI qualification requires actual target observations")
    metadata = run_checked(
        [str(binary), "capabilities", "--json"], stdin=b"", cwd=binary.parent, runner=runner,
    )
    catalog = _qualified_cli_catalog(json.loads(metadata.stdout), qualification)
    if sha256_file(binary) != digest:
        raise RuntimeError("CLI executable changed during qualification")
    return {
        "executable_sha256": digest,
        "render_config": config,
        "svg_pipeline": "resvg-safe",
        "matched_outputs": matched,
        "catalog": catalog,
    }


def _qualified_cli_catalog(capabilities: dict, qualification: dict) -> dict:
    """Join fresh runner-owned scopes to the exact production metadata just observed.

    This is a projection inside collect(), not an importer or qualification issuer. Stored
    records are only accepted by rerunning collect() and comparing the complete result.
    """
    baseline = qualification["catalog"]
    validate_unqualified_catalog(baseline)
    validate_unqualified_catalog(capabilities.get("theme_presets"))
    if capabilities.get("theme_presets") != baseline:
        raise RuntimeError("CLI preset catalog differs from the candidate renderer")
    catalog = copy.deepcopy(baseline)
    entries = {entry["id"]: entry for entry in catalog["presets"]}
    seen = set()
    for preset in qualification["presets"]:
        preset_id = preset["preset"]
        if preset_id in seen or preset_id not in entries:
            raise RuntimeError("Unknown or duplicate qualified preset catalog scope")
        seen.add(preset_id)
        base = entries[preset_id]
        if not base["available"]:
            raise RuntimeError("Qualified preset is unavailable in CLI catalog")
        profile = preset.get("profile")
        if profile != QUALIFIED_PROFILE:
            raise RuntimeError("Unknown or unsupported qualification profile")
        cells = []
        for cell in preset["cells"]:
            family = cell.get("family")
            if family not in QUALIFIED_FAMILIES:
                raise RuntimeError("Unknown or unsupported qualification family")
            output = cell.get("output")
            if output not in QUALIFIED_OUTPUTS:
                raise RuntimeError("Unknown or unsupported qualification output")
            admission = cell.get("admission")
            if admission != QUALIFIED_ADMISSION:
                raise RuntimeError("Unknown or unsupported qualification admission status")
            cells.append({
                "family_id": family, "output_id": output,
                "profile_id": profile, "admission_status": admission,
            })
        cells.sort(key=lambda cell: (cell["family_id"], cell["output_id"], cell["profile_id"]))
        scopes = {(cell["family_id"], cell["output_id"]) for cell in cells}
        if not cells or len(scopes) != len(cells):
            raise RuntimeError("Missing or duplicate qualified catalog scope")
        expected = {**base, "qualified_cells": cells}
        if preset["catalog_entry"] != expected:
            raise RuntimeError("Qualified catalog scope differs from actual target observations")
        base["qualified_cells"] = cells
    return catalog


def collect(root: Path, *, cli_binary: Path | None = None) -> dict:
    revision = clean_revision(root)
    lock_digest = sha256_file(root / "Cargo.lock")
    env = acceptance_environment(dict(os.environ))
    build = subprocess.run(BUILD, cwd=root, env=env, check=True, capture_output=True, text=True)
    # Cargo sends diagnostics to stderr and artifact identities as JSON lines to stdout.
    sys.stderr.write(build.stderr)
    artifacts = [json.loads(line) for line in build.stdout.splitlines() if line.strip()]
    executables = {
        item["executable"] for item in artifacts
        if item.get("reason") == "compiler-artifact"
        and item.get("target", {}).get("name") == "preset_qualification"
        and "example" in item.get("target", {}).get("kind", [])
        and item.get("executable")
    }
    if len(executables) != 1:
        raise RuntimeError("Cargo did not identify exactly one preset qualification executable")
    executable = Path(executables.pop())
    executable_digest = sha256_file(executable)
    execution = subprocess.run(
        [str(executable)], cwd=root, env=env, check=True, capture_output=True, text=True,
    )
    result = json.loads(execution.stdout)
    cli = qualify_cli(cli_binary, result) if cli_binary is not None else None
    if (clean_revision(root) != revision or sha256_file(root / "Cargo.lock") != lock_digest
            or sha256_file(executable) != executable_digest):
        raise RuntimeError("Candidate source, lockfile, or executable changed during qualification")
    return {
        "schema_version": 3,
        "source_commit": revision,
        "lockfile_sha256": lock_digest,
        "executable_sha256": executable_digest,
        "build_command": BUILD,
        "rustc": subprocess.check_output(["rustc", "-Vv"], cwd=root, text=True).strip(),
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "qualification": result,
        "cli": cli,
    }


def archive_catalog(record: dict) -> dict:
    """Project public archive metadata from the freshly replayed native execution record.

    This projection does not validate or issue qualification. Release assembly only accepts its
    input from the successful native verification job and binds it to the verified archive.
    """
    return {
        "schema_version": 1,
        "source_commit": record["source_commit"],
        "artifact": {**record["cli_archive"],
                     "executable_sha256": record["cli"]["executable_sha256"]},
        "host": record["host"],
        "catalog": record["cli"]["catalog"],
        "render_config": record["cli"]["render_config"],
        "svg_pipeline": record["cli"]["svg_pipeline"],
        "qualifications": [{
            **{key: preset[key] for key in (
                "preset", "profile", "qualification_schema_revision",
                "recipe_fingerprint", "resource_fingerprint",
            )},
            "scenarios": [{key: cell[key] for key in (
                "family", "output", "source_id", "source_digest", "png_scale", "font_source",
            )} for cell in preset["cells"]],
        } for preset in record["qualification"]["presets"]],
    }


def verify_record(expected: dict, actual: dict) -> None:
    # Python equality aliases bool/int and int/float; JSON types are part of the record.
    if json.dumps(expected, sort_keys=True, allow_nan=False) != json.dumps(
        actual, sort_keys=True, allow_nan=False,
    ):
        raise RuntimeError("Stale qualification: candidate build, host profile, or execution evidence differs")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--output", type=Path, help="write a new execution record, normally under target/")
    mode.add_argument("--check", type=Path, help="rebuild and reexecute; reject any record mismatch")
    parser.add_argument("--cli", type=Path, help="also match this production CLI against qualified outputs")
    args = parser.parse_args()
    try:
        expected = json.loads(args.check.read_text(encoding="utf-8")) if args.check else None
        record = collect(ROOT, cli_binary=args.cli)
        if args.check:
            verify_record(expected, record)
            print("Scoped preset qualification matches the clean candidate build and execution")
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
            print(f"Recorded scoped preset qualification: {args.output}")
        return 0
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
        print(f"Preset qualification failed: {error}", file=sys.stderr)
        if isinstance(error, subprocess.CalledProcessError) and error.stderr:
            print(error.stderr, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

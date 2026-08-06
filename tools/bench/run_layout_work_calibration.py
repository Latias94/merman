#!/usr/bin/env python3
"""Run the decision-grade interactive layout-work calibration process matrix."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Sequence


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = Path(__file__).resolve()


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def command_output(command: Sequence[str]) -> str:
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    return completed.stdout.strip()


def require_clean_tracked_worktree() -> dict[str, str]:
    status = command_output(["git", "status", "--porcelain", "--untracked-files=no"])
    if status:
        raise RuntimeError("tracked worktree must be clean before calibration")
    command_output(
        [
            "git",
            "ls-files",
            "--error-unmatch",
            str(SCRIPT_PATH.relative_to(ROOT)),
        ]
    )
    return {
        "git_revision": command_output(["git", "rev-parse", "HEAD"]),
        "git_tree": command_output(["git", "rev-parse", "HEAD^{tree}"]),
    }


def timing_command(binary: Path, arguments: Sequence[str]) -> tuple[list[str], str]:
    system = platform.system()
    if system == "Darwin":
        return ["/usr/bin/time", "-l", str(binary), *arguments], "darwin-time-l"
    if system == "Linux":
        return ["/usr/bin/time", "-v", str(binary), *arguments], "gnu-time-v"
    raise RuntimeError(f"peak-RSS calibration is unsupported on {system}")


def parse_peak_rss_bytes(stderr: str, timing_format: str) -> int:
    if timing_format == "darwin-time-l":
        match = re.search(
            r"^\s*(\d+)\s+maximum resident set size$", stderr, re.MULTILINE
        )
        if match:
            return int(match.group(1))
    elif timing_format == "gnu-time-v":
        match = re.search(
            r"^\s*Maximum resident set size \(kbytes\):\s*(\d+)\s*$",
            stderr,
            re.MULTILINE,
        )
        if match:
            return int(match.group(1)) * 1024
    raise RuntimeError("unable to parse maximum resident set size")


def host_report() -> dict[str, Any]:
    report: dict[str, Any] = {
        "platform": platform.platform(),
        "system": platform.system(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "uname": command_output(["uname", "-srm"]),
    }
    if platform.system() == "Darwin":
        report["model"] = command_output(["sysctl", "-n", "hw.model"])
        report["memory_bytes"] = int(command_output(["sysctl", "-n", "hw.memsize"]))
    elif platform.system() == "Linux":
        meminfo = Path("/proc/meminfo").read_text(encoding="utf-8")
        match = re.search(r"^MemTotal:\s*(\d+)\s+kB$", meminfo, re.MULTILINE)
        report["memory_bytes"] = int(match.group(1)) * 1024 if match else None
    return report


def run_one(
    *,
    name: str,
    binary: Path,
    common_arguments: Sequence[str],
    arguments: Sequence[str],
    out_dir: Path,
    timeout_seconds: int,
) -> dict[str, Any]:
    report_path = out_dir / f"{name}.json"
    stdout_path = out_dir / f"{name}.stdout.txt"
    stderr_path = out_dir / f"{name}.time.txt"
    command, timing_format = timing_command(
        binary,
        [*common_arguments, "--json-out", str(report_path), *arguments],
    )
    started = time.monotonic()
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            timeout=timeout_seconds,
        )
        timed_out = False
    except subprocess.TimeoutExpired as error:
        elapsed = time.monotonic() - started
        stdout = error.stdout if isinstance(error.stdout, str) else ""
        stderr = error.stderr if isinstance(error.stderr, str) else ""
        stdout_path.write_text(stdout, encoding="utf-8")
        stderr_path.write_text(stderr, encoding="utf-8")
        return {
            "name": name,
            "command": command,
            "returncode": None,
            "timed_out": True,
            "timeout_seconds": timeout_seconds,
            "elapsed_seconds": elapsed,
            "stdout_path": str(stdout_path.relative_to(ROOT)),
            "stdout_sha256": sha256_file(stdout_path),
            "stderr_path": str(stderr_path.relative_to(ROOT)),
            "stderr_sha256": sha256_file(stderr_path),
        }

    elapsed = time.monotonic() - started
    stdout_path.write_text(completed.stdout, encoding="utf-8")
    stderr_path.write_text(completed.stderr, encoding="utf-8")
    if completed.returncode != 0:
        raise RuntimeError(
            f"{name} failed with exit {completed.returncode}: {completed.stderr.strip()}"
        )
    if not report_path.is_file():
        raise RuntimeError(f"{name} did not create {report_path}")

    payload = report_path.read_bytes()
    return {
        "name": name,
        "command": command,
        "returncode": completed.returncode,
        "timed_out": timed_out,
        "timeout_seconds": timeout_seconds,
        "elapsed_seconds": elapsed,
        "maximum_resident_set_size_bytes": parse_peak_rss_bytes(
            completed.stderr, timing_format
        ),
        "timing_format": timing_format,
        "report_path": str(report_path.relative_to(ROOT)),
        "report_bytes": len(payload),
        "report_sha256": sha256_bytes(payload),
        "stdout_path": str(stdout_path.relative_to(ROOT)),
        "stdout_sha256": sha256_file(stdout_path),
        "stderr_path": str(stderr_path.relative_to(ROOT)),
        "stderr_sha256": sha256_file(stderr_path),
    }


def validate_report_provenance(
    report_path: Path, *, git_revision: str, executable_sha256: str
) -> dict[str, Any]:
    report = json.loads(report_path.read_text(encoding="utf-8"))
    provenance = report["provenance"]
    if provenance["git_revision"] != git_revision:
        raise RuntimeError(f"{report_path}: Git revision differs")
    if provenance["executable_sha256"] != executable_sha256:
        raise RuntimeError(f"{report_path}: executable digest differs")
    if not (
        provenance["tracked_worktree_clean"]
        and provenance["owned_inputs_tracked"]
        and provenance["postflight_identical"]
    ):
        raise RuntimeError(f"{report_path}: fail-closed provenance did not pass")
    return report


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--authoritative-date", required=True)
    parser.add_argument(
        "--binary",
        type=Path,
        default=ROOT / "target/release/examples/layout_work_calibration",
    )
    parser.add_argument("--corpus", default="tools/bench/corpus.json")
    parser.add_argument(
        "--out-dir",
        type=Path,
        default=ROOT / "target/bench/layout-work-calibration",
    )
    parser.add_argument("--timeout-seconds", type=int, default=300)
    parser.add_argument("--full-repeats", type=int, default=5)
    return parser.parse_args()


def main() -> int:
    args = parse_arguments()
    if args.timeout_seconds <= 0:
        raise RuntimeError("--timeout-seconds must be positive")
    if args.full_repeats < 2:
        raise RuntimeError("--full-repeats must be at least two")

    source = require_clean_tracked_worktree()
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise RuntimeError(f"release calibration binary is missing: {binary}")
    executable_sha256 = sha256_file(binary)
    out_dir = args.out_dir.resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    common_arguments = [
        "--authoritative-date",
        args.authoritative_date,
        "--corpus",
        args.corpus,
    ]

    runs: list[dict[str, Any]] = []
    for index in range(1, args.full_repeats + 1):
        runs.append(
            run_one(
                name=f"full-{index}",
                binary=binary,
                common_arguments=common_arguments,
                arguments=[
                    "--expected-max-fixture",
                    "flowchart_large",
                    "--boundary-max-nodes",
                    "16384",
                    "--boundary-max-iterations",
                    "65536",
                ],
                out_dir=out_dir,
                timeout_seconds=args.timeout_seconds,
            )
        )

    full_hashes = {run["report_sha256"] for run in runs}
    if len(full_hashes) != 1:
        raise RuntimeError("fresh-process full calibration reports are not byte-identical")
    full_report_path = ROOT / runs[0]["report_path"]
    full_report = validate_report_provenance(
        full_report_path,
        git_revision=source["git_revision"],
        executable_sha256=executable_sha256,
    )
    fixture_corpus = full_report["fixture_corpus"]
    boundary = full_report["cardinality_boundary"]
    maximum_fixture = fixture_corpus["maximum_layout_work_fixture"]
    accepted_nodes = boundary["accepted"]["nodes"]
    rejected_nodes = boundary["rejected"]["nodes"]
    rejected_limit = fixture_corpus["exact_limit_check"]["rejected_limit"]

    probes = [
        ("max-semantic", ["--probe-fixture", maximum_fixture, "--probe-stage", "semantic"]),
        ("max-layout", ["--probe-fixture", maximum_fixture, "--probe-stage", "layout"]),
        ("max-svg", ["--probe-fixture", maximum_fixture, "--probe-stage", "svg"]),
        (
            "max-end-to-end",
            ["--probe-fixture", maximum_fixture, "--probe-stage", "end-to-end"],
        ),
        (
            f"cardinality-{accepted_nodes}",
            [
                "--probe-flowchart-nodes",
                str(accepted_nodes),
                "--probe-stage",
                "end-to-end",
            ],
        ),
        (
            f"cardinality-{rejected_nodes}",
            [
                "--probe-flowchart-nodes",
                str(rejected_nodes),
                "--probe-stage",
                "end-to-end",
            ],
        ),
        (
            "max-w-minus-one",
            [
                "--probe-fixture",
                maximum_fixture,
                "--probe-stage",
                "end-to-end",
                "--probe-limit",
                str(rejected_limit),
            ],
        ),
    ]
    for name, probe_arguments in probes:
        run = run_one(
            name=name,
            binary=binary,
            common_arguments=common_arguments,
            arguments=probe_arguments,
            out_dir=out_dir,
            timeout_seconds=args.timeout_seconds,
        )
        validate_report_provenance(
            ROOT / run["report_path"],
            git_revision=source["git_revision"],
            executable_sha256=executable_sha256,
        )
        runs.append(run)

    stderr_bundle = "\n".join(
        f"{run['name']}\0{run['stderr_sha256']}" for run in runs
    ).encode("utf-8")
    summary = {
        "schema_version": 1,
        "authoritative_date": args.authoritative_date,
        "source": source,
        "runner": {
            "path": str(SCRIPT_PATH.relative_to(ROOT)),
            "sha256": sha256_file(SCRIPT_PATH),
            "python": sys.version,
            "argv": sys.argv,
        },
        "binary": str(binary.relative_to(ROOT)),
        "executable_sha256": executable_sha256,
        "host": host_report(),
        "timeout_seconds": args.timeout_seconds,
        "full_repeats": args.full_repeats,
        "full_reports_byte_identical": True,
        "full_report_sha256": next(iter(full_hashes)),
        "stderr_bundle_sha256": sha256_bytes(stderr_bundle),
        "runs": runs,
    }
    summary_path = out_dir / "run-summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(summary_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

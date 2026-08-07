#!/usr/bin/env python3
"""Run the decision-grade interactive layout-work calibration process matrix."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Sequence


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = Path(__file__).resolve()
TERMINATION_GRACE_SECONDS = 2.0


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
        report["chip"] = command_output(["sysctl", "-n", "machdep.cpu.brand_string"])
        report["model"] = command_output(["sysctl", "-n", "hw.model"])
        report["memory_bytes"] = int(command_output(["sysctl", "-n", "hw.memsize"]))
    elif platform.system() == "Linux":
        meminfo = Path("/proc/meminfo").read_text(encoding="utf-8")
        match = re.search(r"^MemTotal:\s*(\d+)\s+kB$", meminfo, re.MULTILINE)
        report["memory_bytes"] = int(match.group(1)) * 1024 if match else None
    return report


def terminate_process_group(
    process: subprocess.Popen[str], *, grace_seconds: float = TERMINATION_GRACE_SECONDS
) -> tuple[str, str]:
    process_group = process.pid
    try:
        os.killpg(process_group, signal.SIGTERM)
    except ProcessLookupError:
        if process.poll() is None:
            process.terminate()

    deadline = time.monotonic() + grace_seconds
    group_exists = True
    while time.monotonic() < deadline:
        try:
            os.killpg(process_group, 0)
        except ProcessLookupError:
            group_exists = False
            break
        except PermissionError:
            pass
        time.sleep(0.01)

    if group_exists:
        try:
            os.killpg(process_group, signal.SIGKILL)
        except ProcessLookupError:
            pass

    try:
        return process.communicate(timeout=grace_seconds)
    except subprocess.TimeoutExpired as error:
        if process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()
        raise RuntimeError(
            "managed process group terminated but inherited output pipes remained open"
        ) from error


def run_managed_process(
    command: Sequence[str],
    *,
    cwd: Path,
    timeout_seconds: float,
    termination_grace_seconds: float = TERMINATION_GRACE_SECONDS,
) -> dict[str, Any]:
    process = subprocess.Popen(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout_seconds)
        return {
            "returncode": process.returncode,
            "timed_out": False,
            "stdout": stdout,
            "stderr": stderr,
        }
    except subprocess.TimeoutExpired:
        stdout, stderr = terminate_process_group(
            process, grace_seconds=termination_grace_seconds
        )
        return {
            "returncode": process.returncode,
            "timed_out": True,
            "stdout": stdout,
            "stderr": stderr,
        }


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
    completed = run_managed_process(command, cwd=ROOT, timeout_seconds=timeout_seconds)

    elapsed = time.monotonic() - started
    stdout_path.write_text(completed["stdout"], encoding="utf-8")
    stderr_path.write_text(completed["stderr"], encoding="utf-8")
    if completed["timed_out"]:
        raise RuntimeError(
            f"{name} exceeded the {timeout_seconds}-second timeout; the managed process group was terminated"
        )
    if completed["returncode"] != 0:
        raise RuntimeError(
            f"{name} failed with exit {completed['returncode']}: {completed['stderr'].strip()}"
        )
    if not report_path.is_file():
        raise RuntimeError(f"{name} did not create {report_path}")

    payload = report_path.read_bytes()
    return {
        "name": name,
        "command": command,
        "returncode": completed["returncode"],
        "timed_out": completed["timed_out"],
        "timeout_seconds": timeout_seconds,
        "elapsed_seconds": elapsed,
        "maximum_resident_set_size_bytes": parse_peak_rss_bytes(
            completed["stderr"], timing_format
        ),
        "timing_format": timing_format,
        "report_path": str(report_path.relative_to(ROOT)),
        "report_bytes": len(payload),
        "report_sha256": sha256_bytes(payload),
        "stdout_path": str(stdout_path.relative_to(ROOT)),
        "stdout_bytes": stdout_path.stat().st_size,
        "stdout_sha256": sha256_file(stdout_path),
        "stderr_path": str(stderr_path.relative_to(ROOT)),
        "stderr_bytes": stderr_path.stat().st_size,
        "stderr_sha256": sha256_file(stderr_path),
    }


def validate_report_provenance(
    report_path: Path,
    *,
    git_revision: str,
    executable_sha256: str,
    expected_provenance: dict[str, Any] | None = None,
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
    if expected_provenance is not None and provenance != expected_provenance:
        raise RuntimeError(f"{report_path}: provenance differs from the full report")
    return report


def require_fields(actual: dict[str, Any], expected: dict[str, Any], label: str) -> None:
    for key, value in expected.items():
        if actual.get(key) != value:
            raise RuntimeError(
                f"{label}: expected {key}={value!r}, observed {actual.get(key)!r}"
            )


def require_sha256(value: Any, label: str) -> None:
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise RuntimeError(f"{label}: expected a lowercase SHA-256 digest")


def validate_full_report_contract(report: dict[str, Any]) -> None:
    boundary = report["cardinality_boundary"]
    accepted = boundary["accepted"]
    rejected = boundary["rejected"]
    if boundary["scan_start_nodes"] != 1:
        raise RuntimeError("cardinality scan must start at one node")
    if boundary["scanned_through_nodes"] != rejected["nodes"]:
        raise RuntimeError("cardinality scan end must be the first rejected node")
    if boundary["first_rejected_nodes"] != rejected["nodes"]:
        raise RuntimeError("cardinality first-rejection field differs from the payload")
    if accepted["nodes"] + 1 != rejected["nodes"]:
        raise RuntimeError("cardinality accepted prefix is not adjacent to the rejection")
    if boundary["accepted_prefix_count"] != accepted["nodes"]:
        raise RuntimeError("cardinality accepted-prefix count is incomplete")
    if boundary["accepted_prefix_digest_encoding"] != (
        "repeated u64-le(nodes) || u64-le(layout_work_units)"
    ):
        raise RuntimeError("cardinality digest encoding is unexpected")
    require_sha256(
        boundary["accepted_prefix_observations_sha256"],
        "cardinality accepted-prefix observations",
    )


def fixture_probe_input(fixture: dict[str, Any]) -> dict[str, Any]:
    return {
        "kind": "fixture",
        "name": fixture["name"],
        "source_path": fixture["source_path"],
        "nodes": None,
        "edges": None,
        "source_sha256": fixture["source_sha256"],
        "source_bytes": fixture["source_bytes"],
    }


def cardinality_probe_input(point: dict[str, Any]) -> dict[str, Any]:
    return {
        "kind": "linear_flowchart",
        "name": f"linear_flowchart_{point['nodes']}",
        "source_path": None,
        "nodes": point["nodes"],
        "edges": point["edges"],
        "source_sha256": point["source_sha256"],
        "source_bytes": point["source_bytes"],
    }


def accepted_svg_outcome(point: dict[str, Any]) -> dict[str, Any]:
    return {
        "status": "accepted_svg",
        "layout_work_units": point["layout_work_units"],
        "svg_sha256": point["svg_sha256"],
        "svg_bytes": point["svg_bytes"],
        "svg_elements": point["svg_elements"],
    }


def validate_single_probe_contract(
    name: str, report: dict[str, Any], full_report: dict[str, Any]
) -> None:
    if report.get("report_kind") != "single_probe":
        raise RuntimeError(f"{name}: expected a single-probe report")
    corpus = full_report["fixture_corpus"]
    boundary = full_report["cardinality_boundary"]
    maximum_fixture = next(
        fixture
        for fixture in corpus["fixtures"]
        if fixture["name"] == corpus["maximum_layout_work_fixture"]
    )
    exact = corpus["exact_limit_check"]
    expected_fixture_input = fixture_probe_input(maximum_fixture)
    outcome = report["outcome"]

    if name == "max-semantic":
        require_fields(report["input"], expected_fixture_input, name)
        require_fields(
            report,
            {"stage": "semantic"},
            name,
        )
        require_fields(
            outcome,
            {
                "status": "accepted_semantic",
                "semantic_kind": "flowchart",
                "diagram_type": "flowchart-v2",
            },
            name,
        )
    elif name == "max-layout":
        require_fields(report["input"], expected_fixture_input, name)
        require_fields(report, {"stage": "layout"}, name)
        require_fields(outcome, {"status": "accepted_layout"}, name)
        require_sha256(outcome.get("layout_json_sha256"), name)
        if not isinstance(outcome.get("layout_json_bytes"), int) or outcome[
            "layout_json_bytes"
        ] <= 0:
            raise RuntimeError(f"{name}: layout JSON must be non-empty")
    elif name in {"max-svg", "max-end-to-end"}:
        require_fields(report["input"], expected_fixture_input, name)
        require_fields(
            report,
            {"stage": "svg" if name == "max-svg" else "end_to_end"},
            name,
        )
        require_fields(outcome, accepted_svg_outcome(maximum_fixture), name)
    elif name == f"cardinality-{boundary['accepted']['nodes']}":
        require_fields(
            report["input"], cardinality_probe_input(boundary["accepted"]), name
        )
        require_fields(report, {"stage": "end_to_end"}, name)
        require_fields(outcome, accepted_svg_outcome(boundary["accepted"]), name)
    elif name == f"cardinality-{boundary['rejected']['nodes']}":
        require_fields(
            report["input"], cardinality_probe_input(boundary["rejected"]), name
        )
        require_fields(report, {"stage": "end_to_end"}, name)
        require_fields(
            outcome,
            {"status": "rejected", "rejection": boundary["rejected"]["rejection"]},
            name,
        )
    elif name == "max-w-minus-one":
        require_fields(report["input"], expected_fixture_input, name)
        require_fields(report, {"stage": "end_to_end"}, name)
        require_fields(
            report["policy"],
            {
                "max_layout_work_units": exact["rejected_limit"],
                "explicit_overrides": [
                    {
                        "id": "max_layout_work_units",
                        "value": exact["rejected_limit"],
                    }
                ],
            },
            name,
        )
        require_fields(
            outcome,
            {"status": "rejected", "rejection": exact["rejection"]},
            name,
        )
    else:
        raise RuntimeError(f"unexpected calibration probe: {name}")

    if not isinstance(outcome.get("elapsed_ns"), int) or outcome["elapsed_ns"] <= 0:
        raise RuntimeError(f"{name}: internal elapsed time must be positive")


def prepare_output_directory(path: Path) -> None:
    if path.exists():
        if not path.is_dir():
            raise RuntimeError(f"calibration output path is not a directory: {path}")
        if any(path.iterdir()):
            raise RuntimeError(f"calibration output directory must be empty: {path}")
        return
    path.mkdir(parents=True)


def validate_run_artifacts(run: dict[str, Any]) -> None:
    for kind in ("report", "stdout", "stderr"):
        path = ROOT / run[f"{kind}_path"]
        expected_bytes = run[f"{kind}_bytes"]
        expected_sha256 = run[f"{kind}_sha256"]
        if path.stat().st_size != expected_bytes:
            raise RuntimeError(f"{run['name']}: {kind} byte length changed after capture")
        if sha256_file(path) != expected_sha256:
            raise RuntimeError(f"{run['name']}: {kind} digest changed after capture")


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
    runner_sha256 = sha256_file(SCRIPT_PATH)
    binary = args.binary.resolve()
    if not binary.is_file() or not os.access(binary, os.X_OK):
        raise RuntimeError(f"release calibration binary is missing: {binary}")
    executable_sha256 = sha256_file(binary)
    out_dir = args.out_dir.resolve()
    prepare_output_directory(out_dir)
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
    validate_full_report_contract(full_report)
    expected_provenance = full_report["provenance"]
    for run in runs[1:]:
        validate_report_provenance(
            ROOT / run["report_path"],
            git_revision=source["git_revision"],
            executable_sha256=executable_sha256,
            expected_provenance=expected_provenance,
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
        report = validate_report_provenance(
            ROOT / run["report_path"],
            git_revision=source["git_revision"],
            executable_sha256=executable_sha256,
            expected_provenance=expected_provenance,
        )
        validate_single_probe_contract(name, report, full_report)
        runs.append(run)

    final_source = require_clean_tracked_worktree()
    if final_source != source:
        raise RuntimeError("Git revision or tree changed during calibration")
    if sha256_file(SCRIPT_PATH) != runner_sha256:
        raise RuntimeError("calibration runner changed during calibration")
    if sha256_file(binary) != executable_sha256:
        raise RuntimeError("calibration executable changed during calibration")
    for run in runs:
        validate_run_artifacts(run)

    stderr_index = "\n".join(
        f"{run['name']}\0{run['stderr_sha256']}" for run in runs
    ).encode("utf-8")
    summary = {
        "schema_version": 1,
        "authoritative_date": args.authoritative_date,
        "source": source,
        "runner": {
            "path": str(SCRIPT_PATH.relative_to(ROOT)),
            "sha256": runner_sha256,
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
        "stderr_index_sha256": sha256_bytes(stderr_index),
        "runs": runs,
    }
    summary_path = out_dir / "run-summary.json"
    summary_path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(summary_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

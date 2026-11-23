#!/usr/bin/env python3
"""Measure sequential file-to-file renders, starting a real CLI for every diagram."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import time
import zlib
from concurrent.futures import ThreadPoolExecutor, TimeoutError
from pathlib import Path

if __package__:
    from .run_layout_work_calibration import WindowsJob, windows_peak_working_set
    from .workload_svg import valid_expected_labels, validate_svg
else:
    from run_layout_work_calibration import WindowsJob, windows_peak_working_set
    from workload_svg import valid_expected_labels, validate_svg


def command_prefix(engine: str, binary: str) -> list[str]:
    path = Path(binary)
    if os.name == "nt" and engine == "mmdc" and not path.suffix and path.with_suffix(".cmd").is_file():
        path = path.with_suffix(".cmd")
    if not path.is_file():
        found = shutil.which(binary)
        if not found:
            raise ValueError(f"CLI executable not found: {binary}")
        path = Path(found)
    path = path.resolve()
    launcher_dir = path.parent
    npm_shim = os.name == "nt" and path.suffix.lower() in {".cmd", ".bat"}
    if npm_shim:
        # npm shims forward %* through cmd.exe. Launch the same package bin with
        # Node so paths, diagram IDs and extra arguments stay literal argv entries.
        if engine != "mmdc" or path.stem.lower() != "mmdc":
            raise ValueError("use a native executable or the npm mmdc.cmd shim")
        for modules in (path.parent.parent, path.parent / "node_modules"):
            package = modules / "@mermaid-js" / "mermaid-cli"
            manifest = package / "package.json"
            if manifest.is_file():
                bins = json.loads(manifest.read_bytes())["bin"]
                entry = bins if isinstance(bins, str) else bins["mmdc"]
                path = (package / entry).resolve(strict=True)
                break
        else:
            raise ValueError("cannot resolve npm mmdc.cmd; pass the package's JavaScript bin instead")
    if npm_shim or (engine == "mmdc" and path.suffix.lower() in {".js", ".mjs", ".cjs"}):
        local_node = launcher_dir / "node.exe"
        node = str(local_node) if os.name == "nt" and local_node.is_file() else shutil.which("node")
        if not node:
            raise ValueError("Node.js executable not found for mmdc")
        return [node, str(path)]
    return [str(path)]


def prepare_plan(data: bytes, run_dir: Path, *, svg=True) -> tuple[str | None, list[dict]]:
    plan = json.loads(data)
    version = plan.get("schema_version") if isinstance(plan, dict) else None
    if type(version) is not int or version != 1:
        raise ValueError("expected workload plan schema_version 1")
    scenarios = plan.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise ValueError("expected at least one scenario")
    inputs = run_dir / "inputs"
    inputs.mkdir()
    prepared = []
    seen = set()
    for scenario in scenarios:
        sid = scenario.get("id") if isinstance(scenario, dict) else None
        if not isinstance(sid, str) or not sid or sid in seen:
            raise ValueError(f"missing or duplicate scenario id: {sid!r}")
        seen.add(sid)
        requests = scenario.get("requests")
        if not isinstance(requests, list) or not requests:
            raise ValueError(f"expected requests in scenario {sid!r}")
        rows = []
        for index, request in enumerate(requests):
            fields = ("fixture", "source", "diagram_id")
            if not isinstance(request, dict) or not all(
                isinstance(request.get(key), str) for key in fields
            ):
                raise ValueError(
                    f"invalid fixture, source or diagram_id in {sid!r} request {index + 1}"
                )
            labels = request.get("expected_labels", [])
            if not valid_expected_labels(labels):
                raise ValueError(f"invalid expected_labels in {sid!r} request {index + 1}")
            source = request["source"].encode("utf-8")
            digest = hashlib.sha256(source).hexdigest()
            if request.get("source_sha256", digest) != digest:
                raise ValueError(f"source_sha256 mismatch in {sid!r} request {index + 1}")
            path = inputs / f"{digest}.mmd"
            if not path.exists():
                path.write_bytes(source)
            rows.append(
                {
                    "fixture": request["fixture"],
                    "diagram_id": request["diagram_id"],
                    "input": path.relative_to(run_dir).as_posix(),
                    "source_sha256": digest,
                    "source_bytes": len(source),
                    "expected_labels": labels,
                    **({"drawing_required": True} if svg and "drawing_facts" in request else {}),
                }
            )
        prepared.append(dict(id=sid, requests=rows, batches=[]))
    return plan.get("suite"), prepared


def wait_for_exit(process):
    if os.name == "nt":
        process.wait()
        return {"peak_working_set_bytes": windows_peak_working_set(process)}
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    return {
        "max_rss_bytes": usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024),
        "user_cpu_ms": usage.ru_utime * 1000,
        "system_cpu_ms": usage.ru_stime * 1000,
    }


def windows_job_cpu(job):
    import ctypes
    from ctypes import wintypes

    class Accounting(ctypes.Structure):
        _fields_ = [
            ("user", ctypes.c_int64),
            ("kernel", ctypes.c_int64),
            ("period_user", ctypes.c_int64),
            ("period_kernel", ctypes.c_int64),
            ("page_faults", wintypes.DWORD),
            ("processes", wintypes.DWORD),
            ("active", wintypes.DWORD),
            ("terminated", wintypes.DWORD),
        ]

    counters = Accounting()
    query = job.kernel.QueryInformationJobObject
    query.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
    query.restype = wintypes.BOOL
    if not query(job.handle, 1, ctypes.byref(counters), ctypes.sizeof(counters), None):
        raise ctypes.WinError(ctypes.get_last_error())
    return {
        "user_cpu_ms": counters.user / 10_000,
        "system_cpu_ms": counters.kernel / 10_000,
        "job_processes": counters.processes,
    }


def invoke(command, timeout, run_dir, executor):
    # File-backed diagnostics never block an exiting process on pipe capacity.
    with tempfile.TemporaryFile(dir=run_dir) as log:
        job = WindowsJob() if os.name == "nt" else None
        start = time.perf_counter_ns()
        try:
            with subprocess.Popen(
                command,
                stdin=subprocess.DEVNULL,
                stdout=log,
                stderr=subprocess.STDOUT,
                start_new_session=os.name == "posix",
            ) as process:
                if job:
                    try:
                        job.assign(process)
                    except OSError:
                        process.kill()
                        raise
                waiter = executor.submit(wait_for_exit, process)
                timed_out = False
                try:
                    resources = waiter.result(timeout=timeout)
                except TimeoutError:
                    timed_out = True
                    if job:
                        job.terminate()
                    else:
                        # Give Node a chance to close its detached Chromium process.
                        for sig in (signal.SIGTERM, signal.SIGKILL):
                            try:
                                os.killpg(process.pid, sig)
                            except ProcessLookupError:
                                break
                            if sig == signal.SIGTERM:
                                try:
                                    waiter.result(timeout=1)
                                except TimeoutError:
                                    pass
                    resources = waiter.result()
                elapsed = (time.perf_counter_ns() - start) / 1e6
                if job:
                    resources.update(windows_job_cpu(job))
                status = "exit_error" if process.returncode else "ok"
                if timed_out:
                    status = "timeout"
                result = dict(
                    status=status,
                    returncode=process.returncode,
                    elapsed_ms=elapsed,
                    resources=resources,
                )
        except (OSError, ValueError) as error:
            return dict(
                status="process_error",
                returncode=None,
                error=str(error),
                elapsed_ms=(time.perf_counter_ns() - start) / 1e6,
            )
        finally:
            if job:
                job.close()
        size = log.tell()
        if size:
            log.seek(max(0, size - 2048))
            result["log_tail"] = log.read().decode("utf-8", "replace").strip()
        return result


def png_size(data: bytes) -> tuple[int, int]:
    if not data.startswith(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR") or len(data) < 33:
        raise ValueError("missing PNG signature or IHDR")
    width, height = struct.unpack_from(">II", data, 16)
    if not (0 < width < 2**31 and 0 < height < 2**31):
        raise ValueError(f"invalid PNG dimensions {width} x {height}")
    has_pixels = False
    offset = 8
    view = memoryview(data)
    while offset + 12 <= len(data):
        length, tag = struct.unpack_from(">I4s", data, offset)
        end = offset + 8 + length
        if (
            end + 4 > len(data)
            or zlib.crc32(view[offset + 4 : end]) != struct.unpack_from(">I", data, end)[0]
        ):
            raise ValueError("truncated or corrupt PNG chunk")
        offset = end + 4
        if tag == b"IDAT" and length:
            has_pixels = True
        if tag == b"IEND":
            if has_pixels and length == 0 and offset == len(data):
                return width, height
            break
    raise ValueError("incomplete PNG image")


def verify_output(path: Path, fmt: str, expected_labels=()) -> dict:
    data = path.read_bytes()
    identity = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    if fmt == "png":
        identity["width"], identity["height"] = png_size(data)
    else:
        check = validate_svg(data, expected_labels)
        if check["status"] != "ok":
            raise ValueError(check["message"])
    return identity


def verify(result: dict, run_dir: Path, expected_labels) -> None:
    if result["status"] == "ok":
        path = run_dir / result["output"]
        try:
            result.update(verify_output(path, path.suffix[1:], expected_labels))
        except (OSError, ValueError) as error:
            result.update(status="invalid_output", error=str(error))


def check_batches(scenario, run_dir, render, png_checks):
    for batch in scenario["batches"]:
        for request, result in zip(scenario["requests"], batch["results"]):
            labels = request["expected_labels"]
            verify(result, run_dir, labels)
            if png_checks is None or result["status"] != "ok":
                continue
            key = (request["input"], request["diagram_id"], json.dumps(labels))
            if key not in png_checks:
                check = render(request, f"check-{len(png_checks) + 1}.svg")
                verify(check, run_dir, labels)
                png_checks[key] = check
            check = png_checks[key]
            result["svg_check"] = check["output"]
            if check["status"] != "ok":
                result.update(status="failed_svg_check", error=check.get("error", check["status"]))
        batch["completed"] = sum(row["status"] == "ok" for row in batch["results"])
        batch["failed"] = len(batch["results"]) - batch["completed"]
    first = scenario["batches"][0]["results"][0]
    scenario["first_render_ms"] = first["elapsed_ms"] if first["status"] == "ok" else None
    scenario["batch_samples_ms"] = [
        b["wall_ms"] if not b["failed"] else None for b in scenario["batches"]
    ]
    scenario["completed"] = sum(b["completed"] for b in scenario["batches"])
    scenario["failed"] = sum(b["failed"] for b in scenario["batches"])


def benchmark(args, raw: bytes) -> dict:
    prefix = command_prefix(args.engine, args.binary)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    run_dir = Path(
        tempfile.mkdtemp(prefix=f"{args.engine}-{args.format}-", dir=args.output_dir.resolve())
    )
    suite, scenarios = prepare_plan(raw, run_dir, svg=args.format == "svg")
    font_files = []
    render_prefix = [*prefix, "render"] if args.engine == "merman" else prefix.copy()
    render_prefix.extend(args.extra_arg)
    for file in args.font_file:
        path = file.resolve(strict=True)
        font_files.append(dict(path=str(path), sha256=hashlib.sha256(path.read_bytes()).hexdigest()))
        render_prefix.extend(["--font-file", str(path)])
    resource_scope = (
        {"memory": "CLI process peak working set", "cpu": "Windows job including descendants"}
        if os.name == "nt"
        else {"memory": "wait4 maximum RSS", "cpu": "CLI and descendants accounted for by wait4"}
    )
    environment_keys = (
        "FONTCONFIG_FILE",
        "FONTCONFIG_PATH",
        "PUPPETEER_EXECUTABLE_PATH",
        "PUPPETEER_CACHE_DIR",
    )
    machine = platform.machine().lower()
    arch = {
        "amd64": "x64",
        "x86_64": "x64",
        "i386": "ia32",
        "i686": "ia32",
        "x86": "ia32",
        "aarch64": "arm64",
        "armv7l": "arm",
    }.get(machine, machine)
    report = {
        "suite": suite,
        "binary": args.binary,
        "command_prefix": prefix,
        "process_lifecycle": "fresh-process-per-diagram",
        "samples": args.samples,
        "timeout_seconds": args.timeout_seconds,
        "host": {"hostname": platform.node(), "platform": sys.platform, "arch": arch},
        "resource_scope": resource_scope,
        "extra_args": args.extra_arg,
        "render_environment": {key: os.environ[key] for key in environment_keys if key in os.environ},
        "font_files": font_files,
        "run_dir": str(run_dir),
        "scenarios": scenarios,
    }
    png_checks = {} if args.format == "png" else None
    with ThreadPoolExecutor(max_workers=1) as executor:

        def render(request, filename):
            source = str(run_dir / request["input"])
            output = run_dir / filename
            fmt = output.suffix[1:]
            if args.engine == "merman":
                options = [
                    source,
                    "--format",
                    fmt,
                    "--output",
                    str(output),
                    "--svg-id",
                    request["diagram_id"],
                ]
            else:
                options = ["-i", source, "-o", str(output), "-e", fmt, "-I", request["diagram_id"]]
            result = invoke([*render_prefix, *options], args.timeout_seconds, run_dir, executor)
            result["output"] = filename
            return result

        for scenario_number, scenario in enumerate(scenarios, 1):
            for sample in range(1, args.samples + 1):
                start = time.perf_counter_ns()
                results = [
                    render(request, f"s{scenario_number}-b{sample}-r{index}.{args.format}")
                    for index, request in enumerate(scenario["requests"], 1)
                ]
                scenario["batches"].append(
                    dict(wall_ms=(time.perf_counter_ns() - start) / 1e6, results=results)
                )

        # Postflight runs after *all* timed batches. PNG has no diagram-type
        # metadata, so check an SVG for each distinct successful input and ID.
        for scenario in scenarios:
            check_batches(scenario, run_dir, render, png_checks)
        version = invoke([*prefix, "--version"], args.timeout_seconds, run_dir, executor)
    version_text = version.get("log_tail") if version["status"] == "ok" else None
    report.update(
        version=version_text,
        version_probe=version,
        completed=sum(s["completed"] for s in scenarios),
        failed=sum(s["failed"] for s in scenarios),
    )
    if png_checks is not None:
        report["png_svg_checks"] = list(png_checks.values())
    report["status"] = (
        "failed" if report["failed"] or version["status"] != "ok" or not report["version"] else "ok"
    )
    return report


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan", type=Path, required=True, help="shared JSON workload plan; reads scenarios in order"
    )
    parser.add_argument("--engine", choices=("merman", "mmdc"), required=True)
    parser.add_argument(
        "--binary", required=True, help="CLI path or command on PATH (including npm mmdc.cmd)"
    )
    parser.add_argument("--format", choices=("svg", "png"), default="svg")
    parser.add_argument(
        "--samples", type=int, default=3, help="complete sequential batches per scenario (default: 3)"
    )
    parser.add_argument(
        "--output-dir", type=Path, required=True, help="parent of a fresh run directory"
    )
    parser.add_argument(
        "--timeout-seconds", type=float, default=60, help="per-process timeout (default: 60)"
    )
    parser.add_argument(
        "--font-file",
        type=Path,
        action="append",
        default=[],
        help="Merman native font file; repeatable",
    )
    parser.add_argument(
        "--extra-arg",
        action="append",
        default=[],
        metavar="ARG",
        help="one literal render argument; repeat e.g. --extra-arg=-c --extra-arg=fonts.json",
    )
    args = parser.parse_args(argv)
    report = dict(schema_version=1, surface="cli", engine=args.engine, format=args.format)
    try:
        collector = ("workload_cli.py", "workload_svg.py", "run_layout_work_calibration.py")
        fingerprint = "\n".join(hashlib.sha256(Path(__file__).with_name(f).read_bytes()).hexdigest() for f in collector)
        report["collector_sha256"] = hashlib.sha256(fingerprint.encode("ascii")).hexdigest()
        raw = args.plan.read_bytes()
        report["plan_sha256"] = hashlib.sha256(raw).hexdigest()
        if args.samples < 1 or not math.isfinite(args.timeout_seconds) or args.timeout_seconds <= 0:
            raise ValueError("samples and timeout-seconds must be positive and finite")
        if args.font_file and args.engine != "merman":
            raise ValueError(
                "mmdc uses browser fonts. Pass its font configuration with --extra-arg instead of --font-file"
            )
        report.update(benchmark(args, raw))
    except (OSError, ValueError, KeyError, TypeError) as error:
        report.update(status="failed", error=str(error))
        print(json.dumps(report))
        return 2
    print(json.dumps(report))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())

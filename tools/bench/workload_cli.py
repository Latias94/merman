#!/usr/bin/env python3
"""Measure sequential file-to-file renders, starting a real CLI for every diagram."""

from __future__ import annotations

import argparse
import hashlib
import io
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
    from .workload_svg import EXPECTED_OUTCOMES, valid_expected_labels, validate_rejection, validate_svg
else:
    from run_layout_work_calibration import WindowsJob, windows_peak_working_set
    from workload_svg import EXPECTED_OUTCOMES, valid_expected_labels, validate_rejection, validate_svg


# Bound both the encoded file and Pillow's decoded surfaces before allocating them.
MAX_PNG_BYTES = 64 * 1024 * 1024
MAX_PNG_PIXELS = 4096 * 4096


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
        return [str(Path(node).resolve()), str(path)]
    return [str(path)]


def file_stamp(path):
    stat = path.stat()
    return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns


def command_artifacts(prefix):
    # The public launchers are a single executable or an interpreter followed by its script.
    files = [("executable", Path(prefix[0]))]
    if len(prefix) > 1:
        files.append(("script", Path(prefix[-1])))
    stamps = {}
    for role, path in files:
        try:
            stamps[role] = file_stamp(path)
        except OSError:
            pass
    return files, stamps


def file_evidence(role, path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
        return dict(role=role, path=str(path), bytes=stream.tell(), sha256=digest.hexdigest())


def command_identity(files, stamps, engine):
    identity = dict(scope="command-prefix", status="recorded", files=[])
    for role, path in files:
        try:
            evidence = file_evidence(role, path)
            if file_stamp(path) != stamps.get(role):
                raise ValueError(f"Command file changed or was unavailable at launch: {path}")
            identity["files"].append(evidence)
        except (OSError, ValueError) as error:
            identity.update(status="unknown", reason=str(error))
    if engine == "mmdc":
        # Puppeteer's configuration and CLI options can override its environment.
        browser = identity["browser"] = {"status": "unobserved"}
        configured = os.environ.get("PUPPETEER_EXECUTABLE_PATH")
        if configured:
            try:
                browser["configured_executable"] = file_evidence("configured-browser", Path(configured).resolve())
            except OSError as error:
                browser["reason"] = str(error)
    return identity


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
            outcome = request.get("expected_outcome", "draw")
            if outcome not in EXPECTED_OUTCOMES:
                raise ValueError(f"invalid expected_outcome in {sid!r} request {index + 1}: {outcome!r}")
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
                    "expected_outcome": outcome,
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


def query_windows_job(job, information_class, counters):
    import ctypes
    from ctypes import wintypes

    query = job.kernel.QueryInformationJobObject
    query.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
    query.restype = wintypes.BOOL
    return query(job.handle, information_class, ctypes.byref(counters), ctypes.sizeof(counters), None)


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
    if not query_windows_job(job, 1, counters):
        raise ctypes.WinError(ctypes.get_last_error())
    return {
        "user_cpu_ms": counters.user / 10_000,
        "system_cpu_ms": counters.kernel / 10_000,
        "job_processes": counters.processes,
    }


def windows_job_peak_commit(job):
    """Read PeakJobMemoryUsed from JOBOBJECT_EXTENDED_LIMIT_INFORMATION."""
    import ctypes
    from ctypes import wintypes

    class BasicLimits(ctypes.Structure):
        _fields_ = [
            ("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64), ("flags", wintypes.DWORD),
            ("min_working_set", ctypes.c_size_t), ("max_working_set", ctypes.c_size_t),
            ("process_limit", wintypes.DWORD), ("affinity", ctypes.c_size_t),
            ("priority", wintypes.DWORD), ("scheduling", wintypes.DWORD),
        ]

    class ExtendedLimits(ctypes.Structure):
        _fields_ = [
            ("basic", BasicLimits), ("io_counters", ctypes.c_uint64 * 6),
            ("process_memory_limit", ctypes.c_size_t), ("job_memory_limit", ctypes.c_size_t),
            ("peak_process_memory", ctypes.c_size_t), ("peak_job_memory", ctypes.c_size_t),
        ]

    limits = ExtendedLimits()
    return limits.peak_job_memory if query_windows_job(job, 9, limits) else None


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
                    resources["peak_job_commit_bytes"] = windows_job_peak_commit(job)
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
            if size > 2048 and result["status"] == "exit_error":
                log.seek(0)
                result["log_head"] = log.read(2048).decode("utf-8", "replace").strip()
            log.seek(max(0, size - 2048))
            result["log_tail"] = log.read().decode("utf-8", "replace").strip()
        return result


class PngObserverLimit(ValueError):
    """The observer cannot inspect this artifact within its own decode budget."""


def png_size(data: bytes) -> tuple[int, int]:
    if len(data) > MAX_PNG_BYTES:
        raise PngObserverLimit(f"PNG exceeds {MAX_PNG_BYTES} encoded bytes")
    if not data.startswith(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR") or len(data) < 33:
        raise ValueError("missing PNG signature or IHDR")
    width, height = struct.unpack_from(">II", data, 16)
    if not (0 < width < 2**31 and 0 < height < 2**31):
        raise ValueError(f"invalid PNG dimensions {width} x {height}")
    if width * height > MAX_PNG_PIXELS:
        raise PngObserverLimit(f"PNG exceeds {MAX_PNG_PIXELS} decoded pixels ({width} x {height})")
    has_pixels = False
    offset = 8
    view = memoryview(data)
    while offset + 12 <= len(data):
        length, tag = struct.unpack_from(">I4s", data, offset)
        if tag == b"IHDR" and offset != 8:
            raise ValueError("repeated PNG IHDR would replace the checked dimensions")
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


def png_decoder():
    try:
        from PIL import Image
    except ImportError as error:
        raise ImportError(
            "PNG checks require Pillow. Run python -m pip install -r tools/bench/requirements-png.txt"
        ) from error
    return Image


def _nonuniform_on_white(data: bytes) -> bool:
    """Look for at least two visibly different pixels in the delivered static PNG."""
    image_api = png_decoder()
    with image_api.open(io.BytesIO(data), formats=("PNG",)) as image:
        if image.n_frames != 1:
            raise ValueError("expected a static PNG output")
        image.load()
        if image.mode == "I;16":
            # RGBA conversion clips 16-bit gray to 255. At native precision only the
            # transparent gray sample and white can collapse to the same visible color.
            colors = image.convert("I").getcolors(2)
            transparent = image.info.get("transparency")
            return colors is None or len({65535 if gray == transparent else gray for _, gray in colors}) > 1
        if image.mode in ("1", "L", "RGB") and "transparency" not in image.info:
            return image.getcolors(1) is None
        rgba = image if image.mode == "RGBA" else image.convert("RGBA")
        white = image_api.new("RGBA", image.size, "white")
        return image_api.alpha_composite(white, rgba).getcolors(1) is None


class InvalidArtifact(ValueError):
    def __init__(self, status, message, identity):
        super().__init__(message)
        self.evidence = {"artifact_status": status, **identity}


def verify_output(path: Path, fmt: str, expected_labels=(), expected_outcome="draw") -> dict:
    if expected_outcome not in EXPECTED_OUTCOMES:
        raise ValueError(f"invalid expected_outcome: {expected_outcome!r}")
    with path.open("rb") as stream:
        data = stream.read(MAX_PNG_BYTES + 1) if fmt == "png" else stream.read()
    if fmt == "png" and len(data) > MAX_PNG_BYTES:
        raise PngObserverLimit(f"PNG exceeds {MAX_PNG_BYTES} encoded bytes")
    identity = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    if fmt == "png":
        try:
            identity["width"], identity["height"] = png_size(data)
            identity["nonuniform_on_white"] = _nonuniform_on_white(data)
        except PngObserverLimit:
            raise
        except (OSError, ValueError, SyntaxError, struct.error) as error:
            raise InvalidArtifact("invalid-png", str(error), identity) from error
        if not identity["nonuniform_on_white"] and (expected_outcome == "draw" or expected_labels):
            raise InvalidArtifact("empty-png", "PNG has no nonuniform pixels on white", identity)
        identity["output_class"] = "draw" if identity["nonuniform_on_white"] else "empty"
        if expected_outcome == "reject":
            return dict(identity, status="unexpected-success",
                        error="renderer produced a PNG for an expected rejection")
    else:
        check = validate_svg(data, expected_labels, expected_outcome)
        if check["status"] == "unexpected-success":
            return dict(identity, status=check["status"], error=check["message"])
        if check["status"] != "ok":
            raise InvalidArtifact(check["status"], check["message"], identity)
        identity["output_class"] = check.get("output_class", "draw")
    return identity


def verify(result: dict, run_dir: Path, expected_labels, expected_outcome="draw") -> None:
    result["expected_outcome"] = expected_outcome
    path = run_dir / result["output"]
    if expected_outcome == "reject" and result["status"] == "exit_error" and not path.exists():
        message = result.get("log_head", result.get("log_tail", ""))
        verdict = validate_rejection(result["returncode"], message)
        if verdict["status"] == "expected-rejection":
            result.update(status=verdict["status"], diagnostic=message)
    if result["status"] == "ok":
        try:
            result.update(verify_output(path, path.suffix[1:], expected_labels, expected_outcome))
        except PngObserverLimit as error:
            result.update(status="observer-limit", error=str(error))
        except InvalidArtifact as error:
            result.update(status="invalid_output", error=str(error), **error.evidence)
        except (OSError, ValueError) as error:
            result.update(status="invalid_output", error=str(error))


def check_batches(scenario, run_dir, render, png_checks):
    for batch in scenario["batches"]:
        for request, result in zip(scenario["requests"], batch["results"]):
            labels = request["expected_labels"]
            outcome = request["expected_outcome"]
            verify(result, run_dir, labels, outcome)
            if png_checks is None or result["status"] != "ok":
                continue
            key = (request["input"], request["diagram_id"], json.dumps(labels), outcome)
            if key not in png_checks:
                check = render(request, f"check-{len(png_checks) + 1}.svg")
                verify(check, run_dir, labels, outcome)
                png_checks[key] = check
            check = png_checks[key]
            result["svg_check"] = check["output"]
            if check["status"] != "ok":
                result.update(status="failed_svg_check", error=check.get("error", check["status"]))
        batch["completed"] = sum(row["status"] == "ok" for row in batch["results"])
        batch["expected_rejections"] = sum(row["status"] == "expected-rejection" for row in batch["results"])
        batch["failed"] = len(batch["results"]) - batch["completed"] - batch["expected_rejections"]
    first = scenario["batches"][0]["results"][0]
    scenario["first_render_ms"] = first["elapsed_ms"] if first["status"] == "ok" else None
    scenario["batch_samples_ms"] = [
        b["wall_ms"] if b["completed"] == len(b["results"]) else None for b in scenario["batches"]
    ]
    scenario["completed"] = sum(b["completed"] for b in scenario["batches"])
    scenario["expected_rejections"] = sum(b["expected_rejections"] for b in scenario["batches"])
    scenario["failed"] = sum(b["failed"] for b in scenario["batches"])


def benchmark(args, raw: bytes) -> dict:
    image_api = png_decoder() if args.format == "png" else None
    prefix = command_prefix(args.engine, args.binary)
    files, stamps = command_artifacts(prefix)
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
        {"memory": "CLI process peak working set", "cpu": "Windows job including descendants",
         "peak_job_commit_bytes": "Peak committed bytes across processes assigned to the Windows job, queried at CLI exit"}
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
    if image_api is not None:
        report["png_decoder"] = {"name": "Pillow", "version": image_api.__version__}
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

        # Decode the actual outputs after all timed batches. A separate SVG witnesses labels.
        for scenario in scenarios:
            check_batches(scenario, run_dir, render, png_checks)
        version = invoke([*prefix, "--version"], args.timeout_seconds, run_dir, executor)
    version_text = version.get("log_tail") if version["status"] == "ok" else None
    report.update(
        artifact_identity=command_identity(files, stamps, args.engine),
        version=version_text,
        version_probe=version,
        completed=sum(s["completed"] for s in scenarios),
        expected_rejections=sum(s["expected_rejections"] for s in scenarios),
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
    except (ImportError, OSError, ValueError, KeyError, TypeError) as error:
        report.update(status="failed", error=str(error))
        print(json.dumps(report))
        return 2
    print(json.dumps(report))
    return 0 if report["status"] == "ok" else 1


if __name__ == "__main__":
    sys.exit(main())

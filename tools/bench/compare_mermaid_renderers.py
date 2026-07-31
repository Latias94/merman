#!/usr/bin/env python3
"""
Compare renderer performance, coverage, and benchmark availability.

The harness is corpus-driven by default:
- `tools/bench/corpus.json` says which fixtures belong to each suite.
- Criterion runs are still exact benchmark invocations for stable behavior across Criterion
  versions.
- Markdown is for humans; JSON is the durable artifact for CI, trend dashboards, or later
  quality gates.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import math
import os
import platform
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from corpus_utils import (
    Corpus,
    CorpusFixture,
    compare_mmdr_fixture_inputs,
    load_corpus,
    resolve_merman_fixture_path,
    select_corpus_fixtures,
)


DEFAULT_CORPUS = "tools/bench/corpus.json"
DEFAULT_MARKDOWN_OUT = "target/bench/renderer_comparison.md"
DEFAULT_JSON_OUT = "target/bench/renderer_comparison.json"
DEFAULT_COMMAND_TIMEOUT_SECONDS = 30 * 60
DEFAULT_METADATA_TIMEOUT_SECONDS = 30
MERMAID_JS_MAX_SAMPLES = 10_000
MERMAID_JS_NAVIGATION_TIMEOUT_MS = 30_000
MERMAID_JS_FIXTURE_TIMEOUT_GRACE_MS = 60_000
DEFAULT_QUICK_FILTER = (
    r"end_to_end/(flowchart_tiny|flowchart_medium|flowchart_large|sequence_tiny|"
    r"sequence_medium|state_tiny|state_medium|class_tiny|class_medium)"
)

_ANSI_RE = re.compile(r"\x1b\[[0-9;?]*[A-Za-z]")


def strip_ansi(text: str) -> str:
    return _ANSI_RE.sub("", text)


@dataclass(frozen=True)
class TimeEstimate:
    value: float
    unit: str

    def to_nanos(self) -> float:
        u = strip_ansi(self.unit).strip()
        u = u.replace("μ", "µ")
        if u == "ns":
            return self.value
        if u in ("us", "µs", "μs"):
            return self.value * 1e3
        if u == "ms":
            return self.value * 1e6
        if u == "s":
            return self.value * 1e9
        raise ValueError(f"unknown time unit: {self.unit!r}")


@dataclass(frozen=True)
class CriterionBenchList:
    benches: set[str]
    skipped: dict[str, list[str]]


@dataclass(frozen=True)
class PreparedCriterionRunner:
    executable: Path
    sha256: str


@dataclass(frozen=True)
class ComparisonSnapshot:
    repositories: dict[str, dict[str, Any]]
    files: dict[str, dict[str, object]]
    fixture_inputs: dict[str, dict[str, object]]


def pretty_time(nanos: float) -> str:
    if nanos < 1e3:
        return f"{nanos:.2f} ns"
    if nanos < 1e6:
        return f"{nanos / 1e3:.2f} µs"
    if nanos < 1e9:
        return f"{nanos / 1e6:.2f} ms"
    return f"{nanos / 1e9:.2f} s"


def fmt_ratio(v: float | None) -> str:
    if v is None:
        return "-"
    if not (v > 0) or v == float("inf"):
        return "inf"
    if v < 0.01:
        return "<0.01x"
    if v < 0.1:
        return f"{v:.2f}x"
    return f"{v:.1f}x"


def run(
    cmd: list[str],
    cwd: Path,
    *,
    env: dict[str, str] | None = None,
    timeout_seconds: int = DEFAULT_COMMAND_TIMEOUT_SECONDS,
) -> str:
    proc_env = os.environ.copy()
    if env:
        proc_env.update(env)
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(cwd),
            env=proc_env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            check=False,
            timeout=timeout_seconds,
        )
    except subprocess.TimeoutExpired as error:
        output = error.stdout or ""
        raise RuntimeError(
            f"command timed out after {timeout_seconds}s in {cwd}\n"
            f"$ {' '.join(cmd)}\n\n{output}"
        ) from error
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed (exit {proc.returncode}) in {cwd}\n"
            f"$ {' '.join(cmd)}\n\n{proc.stdout}"
        )
    return proc.stdout


def short_error(value: object, *, max_chars: int = 4000) -> str:
    text = str(value)
    if len(text) <= max_chars:
        return text
    return text[:max_chars] + "\n... <truncated>"


_SKIP_LINE = re.compile(
    r"^\[bench\]\[skip\]\[(?P<group>[A-Za-z0-9_\-]+)\]\s+"
    r"(?P<name>[A-Za-z0-9_\-]+):\s*(?P<reason>.+)$"
)


def parse_skip_lines(text: str) -> dict[str, list[str]]:
    skipped: dict[str, list[str]] = {}
    for raw in text.splitlines():
        line = strip_ansi(raw.rstrip("\r\n"))
        m = _SKIP_LINE.match(line)
        if not m:
            continue
        group = m.group("group")
        name = m.group("name")
        skipped.setdefault(group, []).append(name)
    for k in list(skipped.keys()):
        skipped[k] = sorted(set(skipped[k]))
    return skipped


def merge_skips(*items: dict[str, list[str]]) -> dict[str, list[str]]:
    merged: dict[str, set[str]] = {}
    for item in items:
        for group, names in item.items():
            merged.setdefault(group, set()).update(names)
    return {group: sorted(names) for group, names in sorted(merged.items())}


def git_head(cwd: Path) -> str | None:
    try:
        out = run(["git", "rev-parse", "HEAD"], cwd=cwd).strip()
        return out if out else None
    except Exception:
        return None


def _git_bytes(cwd: Path, args: list[str]) -> bytes:
    try:
        proc = subprocess.run(
            ["git", *args],
            cwd=str(cwd),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
            timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
        )
    except subprocess.TimeoutExpired as error:
        raise ValueError(
            f"git {' '.join(args)} timed out in {cwd}"
        ) from error
    if proc.returncode != 0:
        stderr = proc.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(f"git {' '.join(args)} failed in {cwd}: {stderr}")
    return proc.stdout


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def capture_git_provenance(
    checkout: Path,
    *,
    allow_dirty: bool,
    expected_revision: str | None,
) -> dict[str, Any]:
    checkout = checkout.resolve()
    revision = _git_bytes(checkout, ["rev-parse", "HEAD"]).decode().strip()
    tree = _git_bytes(checkout, ["rev-parse", "HEAD^{tree}"]).decode().strip()
    if expected_revision:
        expected = (
            _git_bytes(checkout, ["rev-parse", "--verify", f"{expected_revision}^{{commit}}"])
            .decode()
            .strip()
        )
        if expected != revision:
            raise ValueError(
                f"expected revision {expected} in {checkout}, found {revision}"
            )

    status = _git_bytes(
        checkout,
        ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )
    dirty_entries = [entry for entry in status.split(b"\0") if entry]
    if dirty_entries and not allow_dirty:
        raise ValueError(
            f"checkout is dirty ({len(dirty_entries)} entries): {checkout}; "
            "use --allow-dirty only for diagnostic evidence"
        )

    diff = _git_bytes(checkout, ["diff", "--binary", "HEAD", "--"])
    untracked = [
        entry
        for entry in _git_bytes(
            checkout,
            ["ls-files", "--others", "--exclude-standard", "-z"],
        ).split(b"\0")
        if entry
    ]
    worktree = hashlib.sha256()
    for value in (revision.encode(), tree.encode(), status, diff):
        worktree.update(len(value).to_bytes(8, "big"))
        worktree.update(value)
    untracked_files: list[dict[str, object]] = []
    for raw_path in sorted(untracked):
        relative = raw_path.decode("utf-8", errors="surrogateescape")
        path = checkout / relative
        if path.is_symlink():
            link = os.readlink(path).encode()
            content_digest = hashlib.sha256(link).hexdigest()
            size = len(link)
        elif path.is_file():
            content_digest = _sha256_file(path)
            size = path.stat().st_size
        else:
            content_digest = "missing"
            size = 0
        worktree.update(raw_path)
        worktree.update(content_digest.encode())
        if len(untracked_files) < 100:
            untracked_files.append(
                {"path": relative, "bytes": size, "sha256": content_digest}
            )

    return {
        "revision": revision,
        "tree": tree,
        "dirty": bool(dirty_entries),
        "dirty_disposition": "explicitly_allowed" if dirty_entries else "clean",
        "dirty_entries": [
            entry.decode("utf-8", errors="replace") for entry in dirty_entries[:100]
        ],
        "dirty_entries_truncated": len(dirty_entries) > 100,
        "tracked_diff_sha256": hashlib.sha256(diff).hexdigest(),
        "untracked_files": untracked_files,
        "untracked_files_total": len(untracked),
        "untracked_files_truncated": len(untracked) > len(untracked_files),
        "worktree_sha256": worktree.hexdigest(),
    }


def snapshot_files(paths: dict[str, Path]) -> dict[str, dict[str, object]]:
    snapshots: dict[str, dict[str, object]] = {}
    for label, path in sorted(paths.items()):
        resolved = path.resolve()
        if not resolved.is_file():
            snapshots[label] = {"path": str(resolved), "exists": False}
            continue
        snapshots[label] = {
            "path": str(resolved),
            "exists": True,
            "bytes": resolved.stat().st_size,
            "sha256": _sha256_file(resolved),
        }
    return snapshots


def locked_mermaid_version(lock_path: Path) -> str | None:
    try:
        data = json.loads(lock_path.read_text(encoding="utf-8"))
        version = (
            (data.get("packages") or {})
            .get("node_modules/mermaid", {})
            .get("version")
        )
        return version.strip() if isinstance(version, str) and version.strip() else None
    except (OSError, json.JSONDecodeError, AttributeError):
        return None


def provenance_verification_errors(
    *,
    before: ComparisonSnapshot,
    after: ComparisonSnapshot,
) -> list[str]:
    errors: list[str] = []
    for label, repo_before in sorted(before.repositories.items()):
        repo_after = after.repositories.get(label)
        if repo_after is None:
            errors.append(f"{label} provenance missing after sampling")
        elif repo_before.get("revision") != repo_after.get("revision"):
            errors.append(f"{label} revision changed during sampling")
        elif repo_before.get("worktree_sha256") != repo_after.get("worktree_sha256"):
            errors.append(f"{label} worktree changed during sampling")
    for label, file_before in sorted(before.files.items()):
        if after.files.get(label) != file_before:
            errors.append(f"{label} changed during sampling")
    if before.fixture_inputs != after.fixture_inputs:
        errors.append("fixture inputs changed during sampling")
    return errors


def rustc_verbose(*, toolchain: str | None = None, cwd: Path | None = None) -> str:
    try:
        command = ["rustc"]
        if toolchain:
            command.append(f"+{toolchain}")
        command.append("-Vv")
        out = subprocess.run(
            command,
            cwd=str(cwd) if cwd is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
            timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
        ).stdout.strip()
        return out
    except Exception:
        return "unknown"


def best_effort_cpu_model() -> str:
    try:
        if sys.platform.startswith("win"):
            out = subprocess.run(
                [
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    "(Get-CimInstance Win32_Processor | Select-Object -First 1 -ExpandProperty Name)",
                ],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout.strip()
            if out:
                return out
        elif sys.platform == "darwin":
            out = subprocess.run(
                ["sysctl", "-n", "machdep.cpu.brand_string"],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout.strip()
            if out:
                return out
        else:
            out = subprocess.run(
                ["lscpu"],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                encoding="utf-8",
                errors="replace",
                check=False,
                timeout=DEFAULT_METADATA_TIMEOUT_SECONDS,
            ).stdout
            for line in out.splitlines():
                if ":" not in line:
                    continue
                k, v = line.split(":", 1)
                if k.strip().lower() == "model name" and v.strip():
                    return v.strip()
    except Exception:
        pass
    return platform.processor() or "unknown"


def expand_filter_to_exact_benches(filter_expr: str) -> list[str]:
    """
    Expand a limited, common "group/(a|b|c)" filter form into exact benchmark names.

    Criterion <=0.5 treats the positional filter argument as a regex, while Criterion >=0.8 treats
    it as a substring match. `mermaid-rs-renderer` currently uses Criterion >=0.8, so a regex-style
    filter like "end_to_end/(a|b)" would match nothing there.
    """
    text = filter_expr.strip()
    m = re.fullmatch(r"(?P<prefix>[A-Za-z0-9_-]+)/\((?P<body>[^)]+)\)", text)
    if not m:
        return [text]

    prefix = m.group("prefix")
    alts = [p.strip() for p in m.group("body").split("|") if p.strip()]
    out: list[str] = []
    for name in alts:
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            return [text]
        out.append(f"{prefix}/{name}")
    return out or [text]


_LINE_NAME_ONLY = re.compile(r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s*$")
_LINE_TIME_ONLY = re.compile(r"^\s*time:\s*\[(?P<body>.+?)\]\s*$")
_LINE_INLINE = re.compile(
    r"^(?P<prefix>[A-Za-z0-9_\-]+)/(?P<name>[A-Za-z0-9_\-]+)\s+"
    r"time:\s*\[(?P<body>.+?)\]\s*$"
)


def parse_criterion_times(text: str, prefix: str) -> dict[str, TimeEstimate]:
    """Parse Criterion output and return mid estimates by benchmark name."""
    times: dict[str, TimeEstimate] = {}
    cur: str | None = None

    for raw in text.splitlines():
        line = strip_ansi(raw.rstrip("\r\n"))

        m_inline = _LINE_INLINE.match(line)
        if m_inline and m_inline.group("prefix") == prefix:
            name = m_inline.group("name")
            estimate = _parse_bracket_time(m_inline.group("body"))
            if estimate is not None:
                times[name] = estimate
            cur = None
            continue

        m_name = _LINE_NAME_ONLY.match(line)
        if m_name and m_name.group("prefix") == prefix:
            cur = m_name.group("name")
            continue

        if cur is not None:
            m_time = _LINE_TIME_ONLY.match(line)
            if m_time:
                estimate = _parse_bracket_time(m_time.group("body"))
                if estimate is not None:
                    times[cur] = estimate
                cur = None

    return times


def _parse_bracket_time(body: str) -> TimeEstimate | None:
    # Criterion prints: "<lo> <unit> <mid> <unit> <hi> <unit>".
    body = strip_ansi(body).strip()
    pairs = re.findall(r"([0-9]+(?:\.[0-9]+)?)\s*([A-Za-zµμ]+)", body)
    if len(pairs) < 2:
        return None
    mid_value, mid_unit = pairs[1]
    try:
        return TimeEstimate(float(mid_value), mid_unit)
    except ValueError:
        return None


_LIST_LINE = re.compile(r"^(?P<bench>[A-Za-z0-9_/-]+):\s*benchmark\s*$")


def criterion_prebuild_command(
    *,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
    toolchain: str | None,
) -> list[str]:
    cmd: list[str] = ["cargo"]
    if toolchain:
        cmd.append(f"+{toolchain}")
    cmd.append("bench")
    if (cwd / "Cargo.lock").exists():
        cmd.append("--locked")
    if package:
        cmd.extend(["-p", package])
    if features:
        cmd.extend(["--features", features])
    cmd.extend(
        [
            "--bench",
            bench_bin,
            "--no-run",
            "--message-format=json-render-diagnostics",
        ]
    )
    return cmd


def parse_bench_executable(cargo_output: str, *, cwd: Path, bench_bin: str) -> Path:
    executables: set[Path] = set()
    for raw in cargo_output.splitlines():
        try:
            message = json.loads(raw)
        except json.JSONDecodeError:
            continue
        if not isinstance(message, dict) or message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target")
        if not isinstance(target, dict):
            continue
        kinds = target.get("kind")
        if (
            target.get("name") != bench_bin
            or not isinstance(kinds, list)
            or "bench" not in kinds
        ):
            continue
        executable = message.get("executable")
        if isinstance(executable, str) and executable:
            path = Path(executable)
            executables.add((path if path.is_absolute() else cwd / path).resolve())

    if not executables:
        raise RuntimeError(f"Cargo did not report an executable for bench {bench_bin!r}.")
    if len(executables) != 1:
        rendered = ", ".join(sorted(str(path) for path in executables))
        raise RuntimeError(
            f"Cargo reported multiple executables for bench {bench_bin!r}: {rendered}"
        )
    return next(iter(executables))


def criterion_executable_sha256(executable: Path) -> str:
    if not executable.is_file():
        raise RuntimeError(f"Criterion executable is missing: {executable}")
    if not os.access(executable, os.X_OK):
        raise RuntimeError(f"Criterion executable is not executable: {executable}")
    return _sha256_file(executable)


def verify_criterion_executable(runner: PreparedCriterionRunner) -> None:
    digest = criterion_executable_sha256(runner.executable)
    if digest != runner.sha256:
        raise RuntimeError(
            f"Criterion executable SHA-256 changed: expected {runner.sha256}, found {digest}"
        )


def prepare_criterion_runner(
    *,
    label: str,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
    env: dict[str, str] | None = None,
    toolchain: str | None = None,
) -> PreparedCriterionRunner:
    cmd = criterion_prebuild_command(
        cwd=cwd,
        bench_bin=bench_bin,
        package=package,
        features=features,
        toolchain=toolchain,
    )
    print("[bench]", label + ":", " ".join(cmd))
    out = run(cmd, cwd=cwd, env=env)
    executable = parse_bench_executable(out, cwd=cwd, bench_bin=bench_bin)
    return PreparedCriterionRunner(
        executable=executable,
        sha256=criterion_executable_sha256(executable),
    )


def run_prepared_criterion(
    runner: PreparedCriterionRunner,
    args: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
) -> str:
    return run([str(runner.executable), "--bench", *args], cwd=cwd, env=env)


def list_criterion_benches(
    *,
    cwd: Path,
    runner: PreparedCriterionRunner,
    env: dict[str, str] | None = None,
) -> CriterionBenchList:
    out = run_prepared_criterion(runner, ["--list"], cwd=cwd, env=env)
    benches: set[str] = set()
    for raw in out.splitlines():
        line = strip_ansi(raw).strip()
        m = _LIST_LINE.match(line)
        if not m:
            continue
        benches.add(m.group("bench"))
    return CriterionBenchList(benches=benches, skipped=parse_skip_lines(out))


def split_exact_bench(exact: str) -> tuple[str, str]:
    if "/" not in exact:
        return "", exact
    return exact.split("/", 1)


def read_fixture_source(repo_root: Path, name: str, fixture: CorpusFixture | None) -> str | None:
    path = resolve_merman_fixture_path(repo_root, name, fixture)
    return path.read_text(encoding="utf-8") if path.exists() else None


def bench_exact(
    *,
    cwd: Path,
    runner: PreparedCriterionRunner,
    exact: str,
    sample_size: int,
    warm_up: int,
    measurement: int,
    env: dict[str, str] | None = None,
) -> str:
    return run_prepared_criterion(
        runner,
        [
            "--noplot",
            "--sample-size",
            str(sample_size),
            "--warm-up-time",
            str(warm_up),
            "--measurement-time",
            str(measurement),
            "--discard-baseline",
            "--exact",
            exact,
        ],
        cwd=cwd,
        env=env,
    )


def run_native_runner(
    *,
    label: str,
    cwd: Path,
    runner: PreparedCriterionRunner,
    exact_benches: list[str],
    bench_list: CriterionBenchList,
    sample_size: int,
    warm_up: int,
    measurement: int,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    skipped_exact = {
        f"{group}/{name}"
        for group, names in bench_list.skipped.items()
        for name in names
    }
    available = [b for b in exact_benches if b in bench_list.benches]
    missing = [
        b for b in exact_benches if b not in bench_list.benches and b not in skipped_exact
    ]
    times_ns: dict[str, float] = {}
    errors: dict[str, str] = {}
    output_skips: dict[str, list[str]] = {}
    executable_status = "verified"

    try:
        verify_criterion_executable(runner)
    except Exception as error:
        executable_status = "failed"
        errors["__runner__"] = short_error(error)

    benches_to_run = available if executable_status == "verified" else []
    for exact in benches_to_run:
        prefix, name = split_exact_bench(exact)
        print(
            "[bench]",
            label + ":",
            f"{runner.executable} --bench ... --exact {exact}",
        )
        try:
            out = bench_exact(
                cwd=cwd,
                runner=runner,
                exact=exact,
                sample_size=sample_size,
                warm_up=warm_up,
                measurement=measurement,
                env=env,
            )
        except Exception as e:
            errors[exact] = short_error(e)
            continue

        output_skips = merge_skips(output_skips, parse_skip_lines(out))
        parsed = parse_criterion_times(out, prefix=prefix)
        estimate = parsed.get(name)
        if estimate is None:
            errors[exact] = "Criterion output did not include a parseable mid estimate."
            continue
        try:
            times_ns[exact] = estimate.to_nanos()
        except Exception as e:
            errors[exact] = short_error(e)

    skipped = merge_skips(bench_list.skipped, output_skips)
    if executable_status == "verified":
        try:
            verify_criterion_executable(runner)
        except Exception as error:
            executable_status = "failed"
            errors["__runner__"] = short_error(error)
    return {
        "label": label,
        "kind": "criterion",
        "available": available,
        "missing": missing,
        "errors": errors,
        "skipped": skipped,
        "times_ns": times_ns,
        "estimate_kind": "criterion_console_mid_point",
        "raw_samples_retained": False,
        "executable": {
            "path": str(runner.executable),
            "sha256": runner.sha256,
            "status": executable_status,
        },
    }


def _validated_preflight_receipt(value: object) -> dict[str, Any]:
    view_box = value.get("view_box") if isinstance(value, dict) else None
    if (
        not isinstance(value, dict)
        or not isinstance(value.get("svg_chars"), int)
        or value["svg_chars"] <= 0
        or not isinstance(value.get("svg_bytes"), int)
        or value["svg_bytes"] <= 0
        or not isinstance(value.get("svg_sha256"), str)
        or re.fullmatch(r"[0-9a-f]{64}", value["svg_sha256"]) is None
        or not isinstance(view_box, list)
        or len(view_box) != 4
        or any(
            isinstance(number, bool)
            or not isinstance(number, (int, float))
            or not math.isfinite(float(number))
            for number in view_box
        )
        or float(view_box[2]) <= 0
        or float(view_box[3]) <= 0
    ):
        raise ValueError("Mermaid JS SVG preflight receipt is invalid.")
    return dict(value)


def _normalized_positive_samples(value: object) -> list[float]:
    if not isinstance(value, list) or not value:
        raise ValueError("Mermaid JS returned no raw timing samples.")
    samples: list[float] = []
    for sample in value:
        if (
            isinstance(sample, bool)
            or not isinstance(sample, (int, float))
            or not math.isfinite(float(sample))
            or float(sample) <= 0
        ):
            raise ValueError(
                "Mermaid JS raw timing samples must be finite positive numbers."
            )
        samples.append(float(sample))
    return samples


def _summarize_samples(samples: list[float]) -> dict[str, float | int]:
    ordered = sorted(samples)
    midpoint = len(ordered) // 2
    median = (
        ordered[midpoint]
        if len(ordered) % 2 == 1
        else (ordered[midpoint - 1] + ordered[midpoint]) / 2
    )

    def nearest_rank(percentile: float) -> float:
        index = max(0, math.ceil(percentile * len(ordered)) - 1)
        return ordered[index]

    return {
        "count": len(ordered),
        "min": ordered[0],
        "median": median,
        "p95": nearest_rank(0.95),
        "p99": nearest_rank(0.99),
        "max": ordered[-1],
    }


def _empty_mermaid_js_result() -> dict[str, Any]:
    return {
        "meta": {},
        "method": {},
        "times_ns": {},
        "samples": {},
        "raw_samples_ns": {},
        "sample_stats_ns": {},
        "preflight": {},
        "termination": {},
        "errors": {},
        "revision": None,
    }


def _validated_mermaid_js_method(value: object) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError("Mermaid JS output has no method object.")
    stop_conditions = value.get("measurement_stop_conditions")
    watchdogs = value.get("watchdogs")
    if not isinstance(stop_conditions, dict) or not isinstance(watchdogs, dict):
        raise ValueError("Mermaid JS method metadata is incomplete.")

    method = {
        "measurement_stop_conditions": {
            "measure_ms": stop_conditions.get("measure_ms"),
            "max_samples": stop_conditions.get("max_samples"),
        },
        "watchdogs": {
            "navigation_timeout_ms": watchdogs.get("navigation_timeout_ms"),
            "fixture_timeout_ms": watchdogs.get("fixture_timeout_ms"),
        },
    }
    for group in method.values():
        if any(
            isinstance(value, bool) or not isinstance(value, int) or value <= 0
            for value in group.values()
        ):
            raise ValueError("Mermaid JS method values must be positive integers.")
    if (
        method["watchdogs"]["fixture_timeout_ms"]
        <= method["measurement_stop_conditions"]["measure_ms"]
    ):
        raise ValueError("Mermaid JS fixture watchdog cannot cover the measurement window.")
    return method


def parse_mermaid_js_output(data: object) -> dict[str, Any]:
    parsed = _empty_mermaid_js_result()
    if not isinstance(data, dict):
        parsed["errors"]["__runner__"] = "Mermaid JS output is not an object."
        return parsed
    if data.get("schema_version") != 3:
        parsed["errors"]["__runner__"] = (
            "Mermaid JS output schema_version must be 3."
        )
        return parsed
    try:
        parsed["method"] = _validated_mermaid_js_method(data.get("method"))
    except ValueError as error:
        parsed["errors"]["__runner__"] = str(error)
        return parsed
    if isinstance(data.get("meta"), dict):
        parsed["meta"] = {
            str(key): str(value) for key, value in data["meta"].items()
        }
    mermaid_version = parsed["meta"].get("mermaid")
    if mermaid_version:
        parsed["revision"] = "mermaid@" + mermaid_version

    results = data.get("results")
    if not isinstance(results, dict):
        parsed["errors"]["__runner__"] = "Mermaid JS output has no results object."
        return parsed
    for raw_name, value in results.items():
        name = str(raw_name)
        exact = f"end_to_end/{name}"
        if not isinstance(value, dict):
            parsed["errors"][exact] = "Mermaid JS result is not an object."
            continue
        if value.get("error"):
            parsed["errors"][exact] = str(value["error"])
            continue
        try:
            preflight = _validated_preflight_receipt(value.get("preflight"))
            samples = _normalized_positive_samples(value.get("times_ns"))
            sample_cap = value.get("sample_cap")
            stop_reason = value.get("stop_reason")
            max_samples = parsed["method"]["measurement_stop_conditions"][
                "max_samples"
            ]
            if (
                isinstance(sample_cap, bool)
                or not isinstance(sample_cap, int)
                or sample_cap != max_samples
            ):
                raise ValueError("Mermaid JS result sample_cap does not match its method.")
            if value.get("samples_truncated") is not False:
                raise ValueError("Mermaid JS result must retain all collected samples.")
            if stop_reason not in {"measurement_time", "max_samples"}:
                raise ValueError("Mermaid JS result has an invalid stop_reason.")
            if len(samples) > sample_cap:
                raise ValueError("Mermaid JS result exceeds its sample_cap.")
            if stop_reason == "max_samples" and len(samples) != sample_cap:
                raise ValueError("Mermaid JS result stopped before reaching sample_cap.")
            if stop_reason == "measurement_time" and len(samples) >= sample_cap:
                raise ValueError("Mermaid JS result reached sample_cap without reporting it.")
        except ValueError as error:
            parsed["errors"][exact] = str(error)
            continue

        stats = _summarize_samples(samples)
        median_ns = float(stats["median"])
        parsed["times_ns"][exact] = median_ns
        parsed["samples"][name] = len(samples)
        parsed["raw_samples_ns"][name] = samples
        parsed["sample_stats_ns"][name] = stats
        parsed["preflight"][name] = preflight
        parsed["termination"][name] = {
            "stop_reason": stop_reason,
            "sample_cap": sample_cap,
            "samples_truncated": False,
        }
    return parsed


def _empty_mermaid_js_runner(
    *,
    skip_reason: str,
    missing: list[str] | None = None,
    skipped: dict[str, list[str]] | None = None,
) -> dict[str, Any]:
    runner = _empty_mermaid_js_result()
    runner.update(
        {
            "label": "Mermaid JS",
            "kind": "browser_warm",
            "available": [],
            "missing": missing or [],
            "skipped": skipped or {},
            "skip_reason": skip_reason,
        }
    )
    return runner


def run_mermaid_js(
    *,
    repo_root: Path,
    mermaid_cli_dir: Path,
    exact_benches: list[str],
    fixtures_by_name: dict[str, CorpusFixture],
    sample_warm_up: int,
    sample_measurement: int,
    skip: bool,
) -> dict[str, Any]:
    end_to_end_names = [
        name for group, name in (split_exact_bench(b) for b in exact_benches) if group == "end_to_end"
    ]
    if skip:
        print("[bench] mermaid-js: skipped (--skip-mermaid-js)")
        return _empty_mermaid_js_runner(
            skip_reason="--skip-mermaid-js",
            skipped={"end_to_end": end_to_end_names},
        )
    if not end_to_end_names:
        print("[bench] mermaid-js: skipped (no end_to_end fixtures requested)")
        return _empty_mermaid_js_runner(
            skip_reason="no end_to_end fixtures requested"
        )
    if not mermaid_cli_dir.exists():
        print("[bench] mermaid-js: skipped (missing tools/mermaid-cli)")
        return _empty_mermaid_js_runner(
            skip_reason=f"missing {mermaid_cli_dir}",
            skipped={"end_to_end": end_to_end_names},
        )

    fixtures: dict[str, str] = {}
    missing: list[str] = []
    for name in end_to_end_names:
        text = read_fixture_source(repo_root, name, fixtures_by_name.get(name))
        if text is None:
            missing.append(f"end_to_end/{name}")
        else:
            fixtures[name] = text

    if not fixtures:
        print("[bench] mermaid-js: skipped (no readable fixtures)")
        return _empty_mermaid_js_runner(
            skip_reason="no readable fixtures", missing=missing
        )

    script = repo_root / "tools" / "bench" / "mermaid_js_bench.cjs"
    warmup_ms = sample_warm_up * 1000
    measurement_ms = sample_measurement * 1000
    fixture_timeout_ms = (
        warmup_ms + measurement_ms + MERMAID_JS_FIXTURE_TIMEOUT_GRACE_MS
    )
    print("[bench] mermaid-js (puppeteer): node", script)
    with tempfile.TemporaryDirectory(prefix="merman-mermaid-js-") as temp_dir:
        bench_in = Path(temp_dir) / "input.json"
        bench_out = Path(temp_dir) / "output.json"
        bench_in.write_text(
            json.dumps(
                {
                    "fixtures": fixtures,
                    "configPath": "mermaid-config.json",
                    "theme": "default",
                    "seed": "1",
                    "width": 800,
                    "warmupMs": warmup_ms,
                    "measureMs": measurement_ms,
                    "maxSamples": MERMAID_JS_MAX_SAMPLES,
                    "navigationTimeoutMs": MERMAID_JS_NAVIGATION_TIMEOUT_MS,
                    "fixtureTimeoutMs": fixture_timeout_ms,
                },
                indent=2,
            ),
            encoding="utf-8",
        )
        try:
            run(
                ["node", str(script), "--in", str(bench_in), "--out", str(bench_out)],
                cwd=mermaid_cli_dir,
            )
            if not bench_out.is_file():
                raise ValueError("Mermaid JS runner did not create its output file.")
            parsed = parse_mermaid_js_output(
                json.loads(bench_out.read_text(encoding="utf-8", errors="replace"))
            )
        except Exception as error:
            parsed = _empty_mermaid_js_result()
            parsed["errors"]["__runner__"] = short_error(error)

    if parsed["revision"] is None:
        version = locked_mermaid_version(mermaid_cli_dir / "package-lock.json")
        if version is not None:
            parsed["revision"] = "mermaid@" + version

    parsed.update(
        {
        "label": "Mermaid JS",
        "kind": "browser_warm",
        "available": [f"end_to_end/{name}" for name in fixtures],
        "missing": missing,
        "skipped": {},
        "skip_reason": None,
        }
    )
    return parsed


def native_status(runner: dict[str, Any], exact: str, name: str) -> str:
    if exact in runner.get("times_ns", {}):
        return "measured"
    if exact in runner.get("errors", {}):
        return "error"
    if exact in runner.get("missing", []):
        return "missing"
    group, fixture_name = split_exact_bench(exact)
    if fixture_name in (runner.get("skipped", {}).get(group) or []):
        return "skipped"
    return "not_run"


def mermaid_js_status(runner: dict[str, Any], exact: str, name: str) -> str:
    group, _ = split_exact_bench(exact)
    if group != "end_to_end":
        return "not_applicable"
    if exact in runner.get("times_ns", {}):
        return "measured"
    if exact in runner.get("errors", {}) or name in runner.get("errors", {}) or "__runner__" in runner.get("errors", {}):
        return "error"
    if exact in runner.get("missing", []) or name in runner.get("missing", []):
        return "missing"
    if name in (runner.get("skipped", {}).get("end_to_end") or []):
        return "skipped"
    if runner.get("skip_reason"):
        return "skipped"
    return "not_run"


def applicable_benches_for_runner(runner: dict[str, Any], exact_benches: list[str]) -> list[str]:
    if runner.get("kind") == "browser_warm":
        return [b for b in exact_benches if split_exact_bench(b)[0] == "end_to_end"]
    return exact_benches


def requested_skip_count(runner: dict[str, Any], exact_benches: list[str]) -> int:
    count = 0
    requested = set(applicable_benches_for_runner(runner, exact_benches))
    for group, names in runner.get("skipped", {}).items():
        for name in names:
            if f"{group}/{name}" in requested:
                count += 1
    return count


def coverage_for_runner(runner: dict[str, Any], exact_benches: list[str]) -> dict[str, int]:
    applicable = applicable_benches_for_runner(runner, exact_benches)
    return {
        "requested": len(applicable),
        "available": len(runner.get("available", [])),
        "measured": len(runner.get("times_ns", {})),
        "missing": len(runner.get("missing", [])),
        "errors": len(runner.get("errors", {})),
        "skipped": requested_skip_count(runner, exact_benches),
    }


def build_rows(
    *,
    exact_benches: list[str],
    fixtures_by_name: dict[str, CorpusFixture],
    merman: dict[str, Any],
    mmdr: dict[str, Any],
    mermaid_js: dict[str, Any],
    fixture_inputs: dict[str, dict[str, object]],
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for exact in exact_benches:
        _, name = split_exact_bench(exact)
        fixture = fixtures_by_name.get(name)
        merman_ns = merman.get("times_ns", {}).get(exact)
        mmdr_ns = mmdr.get("times_ns", {}).get(exact)
        mermaid_js_ns = mermaid_js.get("times_ns", {}).get(exact)
        input_status = fixture_inputs.get(name, {}).get("status", "unknown")
        ratio_mmdr = (
            float(merman_ns) / float(mmdr_ns)
            if input_status == "identical"
            and isinstance(merman_ns, (int, float))
            and isinstance(mmdr_ns, (int, float))
            and mmdr_ns
            else None
        )
        ratio_js = (
            float(merman_ns) / float(mermaid_js_ns)
            if isinstance(merman_ns, (int, float))
            and isinstance(mermaid_js_ns, (int, float))
            and mermaid_js_ns
            else None
        )
        rows.append(
            {
                "benchmark": exact,
                "fixture": name,
                "family": fixture.family if fixture else "unknown",
                "size": fixture.size if fixture else "unknown",
                "category": fixture.category if fixture else "adhoc",
                "features": list(fixture.features) if fixture else [],
                "quality": list(fixture.quality) if fixture else [],
                "input_status": input_status,
                "times_ns": {
                    "merman": merman_ns,
                    "mermaid_rs_renderer": mmdr_ns,
                    "mermaid_js": mermaid_js_ns,
                },
                "status": {
                    "merman": native_status(merman, exact, name),
                    "mermaid_rs_renderer": native_status(mmdr, exact, name),
                    "mermaid_js": mermaid_js_status(mermaid_js, exact, name),
                },
                "ratios": {
                    "merman_over_mermaid_rs_renderer": ratio_mmdr,
                    "merman_over_mermaid_js": ratio_js,
                },
            }
        )
    return rows


def comparison_contract_errors(
    *,
    merman: dict[str, Any],
    mmdr: dict[str, Any],
    mermaid_js: dict[str, Any],
    rows: list[dict[str, Any]],
    require_mermaid_js: bool,
    provenance_errors: list[str],
) -> list[str]:
    errors = list(provenance_errors)
    if merman.get("errors"):
        errors.append(
            f"merman benchmark errors: {', '.join(sorted(merman['errors']))}"
        )
    if merman.get("missing"):
        errors.append(
            f"merman benchmarks missing: {', '.join(sorted(merman['missing']))}"
        )
    if any(merman.get("skipped", {}).values()):
        errors.append("merman skipped one or more requested benchmarks")
    if not merman.get("times_ns"):
        errors.append("merman measured no fixtures")
    if mmdr.get("errors"):
        errors.append(
            "mermaid-rs-renderer benchmark errors: "
            + ", ".join(sorted(mmdr["errors"]))
        )
    if require_mermaid_js:
        if mermaid_js.get("errors"):
            errors.append(
                "Mermaid JS benchmark errors: "
                + ", ".join(sorted(mermaid_js["errors"]))
            )
        if not mermaid_js.get("times_ns"):
            errors.append("Mermaid JS measured no fixtures")
        if mermaid_js.get("missing"):
            errors.append(
                f"Mermaid JS fixtures missing: {', '.join(sorted(mermaid_js['missing']))}"
            )
    comparable = [
        row
        for row in rows
        if isinstance(
            row.get("ratios", {}).get("merman_over_mermaid_rs_renderer"),
            (int, float),
        )
    ]
    if not comparable:
        errors.append("no byte-identical, jointly measured Merman/mmdr fixtures")
    return list(dict.fromkeys(errors))


def geomean(values: Iterable[float]) -> float | None:
    vals = [v for v in values if v > 0 and math.isfinite(v)]
    if not vals:
        return None
    return math.exp(sum(math.log(v) for v in vals) / len(vals))


def build_family_summary(rows: list[dict[str, Any]]) -> list[dict[str, Any]]:
    families: dict[str, dict[str, Any]] = {}
    for row in rows:
        family = str(row.get("family") or "unknown")
        item = families.setdefault(
            family,
            {
                "family": family,
                "fixtures": 0,
                "measured": {"merman": 0, "mermaid_rs_renderer": 0, "mermaid_js": 0},
                "ratios_mmdr": [],
            },
        )
        item["fixtures"] += 1
        for runner in ("merman", "mermaid_rs_renderer", "mermaid_js"):
            if row.get("status", {}).get(runner) == "measured":
                item["measured"][runner] += 1
        ratio_mmdr = row.get("ratios", {}).get("merman_over_mermaid_rs_renderer")
        if isinstance(ratio_mmdr, (int, float)):
            item["ratios_mmdr"].append(float(ratio_mmdr))

    out: list[dict[str, Any]] = []
    for family in sorted(families):
        item = families[family]
        out.append(
            {
                "family": family,
                "fixtures": item["fixtures"],
                "measured": item["measured"],
                "geomean_ratios": {
                    "merman_over_mermaid_rs_renderer": geomean(item["ratios_mmdr"]),
                },
            }
        )
    return out


def build_family_coverage(
    family_summary: list[dict[str, Any]],
) -> dict[str, object]:
    requested = {str(item["family"]) for item in family_summary}
    measured = {
        runner: {
            str(item["family"])
            for item in family_summary
            if item.get("measured", {}).get(runner, 0) > 0
        }
        for runner in ("merman", "mermaid_rs_renderer", "mermaid_js")
    }
    comparable = {
        str(item["family"])
        for item in family_summary
        if isinstance(
            item.get("geomean_ratios", {}).get(
                "merman_over_mermaid_rs_renderer"
            ),
            (int, float),
        )
    }
    return {
        "requested": sorted(requested),
        "requested_count": len(requested),
        "measured": {key: sorted(value) for key, value in measured.items()},
        "measured_count": {key: len(value) for key, value in measured.items()},
        "native_same_byte_comparable": sorted(comparable),
        "native_same_byte_comparable_count": len(comparable),
        "native_not_same_byte_comparable": sorted(requested - comparable),
    }


def write_json_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_markdown(out_path: Path, report: dict[str, Any]) -> None:
    out_path.parent.mkdir(parents=True, exist_ok=True)

    def fmt_rev(label: str, rev: str | None) -> str:
        if rev is None:
            return f"- {label}: unknown"
        return f"- {label}: `{rev}`"

    def fmt_cell(row: dict[str, Any], runner: str) -> str:
        status = row["status"][runner]
        ns = row["times_ns"][runner]
        if status == "measured" and isinstance(ns, (int, float)):
            return pretty_time(float(ns))
        return status.replace("_", " ")

    def fmt_mmdr_ratio(row: dict[str, Any]) -> str:
        input_status = row.get("input_status")
        if input_status == "different":
            return "input differs"
        if input_status not in (None, "identical"):
            return str(input_status).replace("_", " ")
        return fmt_ratio(row["ratios"]["merman_over_mermaid_rs_renderer"])

    def fmt_js_stat(row: dict[str, Any], field: str) -> str:
        stats = (
            report["runners"]["mermaid_js"]
            .get("sample_stats_ns", {})
            .get(row["fixture"], {})
        )
        value = stats.get(field)
        if field == "count":
            return str(value) if isinstance(value, int) else "-"
        return pretty_time(float(value)) if isinstance(value, (int, float)) else "-"

    lines: list[str] = []
    lines.append("# Renderer Performance Comparison")
    lines.append("")
    lines.append("> Generated by `tools/bench/compare_mermaid_renderers.py`.")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    env = report["environment"]
    lines.append(f"- Timestamp: \"{report['generated_at']}\"")
    lines.append(f"- OS: \"{env['os']}\"")
    lines.append(f"- Machine: \"{env['machine']}\"")
    lines.append(f"- CPU: \"{env['cpu']}\"")
    lines.append(f"- Python: \"{env['python']}\"")
    lines.append(f"- mmdr toolchain: \"{env['mmdr_toolchain']}\"")
    js_meta = report["runners"]["mermaid_js"].get("meta", {})
    if js_meta.get("node"):
        lines.append(f"- Node: \"{js_meta['node']}\"")
    if js_meta.get("chromium"):
        lines.append(f"- Chromium: \"{js_meta['chromium']}\"")
    if js_meta.get("puppeteer"):
        lines.append(f"- Puppeteer: \"{js_meta['puppeteer']}\"")
    if js_meta.get("mermaid_cli"):
        lines.append(f"- mermaid-cli: \"{js_meta['mermaid_cli']}\"")
    lines.append(fmt_rev("merman", report["runners"]["merman"].get("revision")))
    lines.append(fmt_rev("mermaid-rs-renderer", report["runners"]["mermaid_rs_renderer"].get("revision")))
    lines.append(fmt_rev("mermaid-js", report["runners"]["mermaid_js"].get("revision")))
    lines.append("- Rust:")
    lines.append("")
    lines.append("```")
    lines.append(env["rust"])
    lines.append("```")
    lines.append("")
    lines.append("- mmdr Rust:")
    lines.append("")
    lines.append("```")
    lines.append(env["mmdr_rust"])
    lines.append("```")
    lines.append("")
    contract = report["contract"]
    provenance = report["provenance"]
    lines.append("## Evidence Status")
    lines.append("")
    lines.append(f"- Evidence class: `{report['method']['evidence_class']}`")
    lines.append(f"- Contract status: `{contract['status']}`")
    lines.append(f"- Baseline eligible: `{str(contract['baseline_eligible']).lower()}`")
    lines.append(
        "- Post-sampling provenance: "
        f"`{provenance['post_sampling']['status']}`"
    )
    for label, repo in provenance["repositories"].items():
        lines.append(
            f"- {label} worktree: `{'dirty' if repo['dirty'] else 'clean'}`; "
            f"fingerprint `{repo['worktree_sha256']}`"
        )
    if contract["errors"]:
        lines.append("- Contract errors:")
        for error in contract["errors"]:
            lines.append(f"  - {error}")
    lines.append("")
    lines.append("## Method")
    lines.append("")
    selection = report["selection"]
    lines.append(f"- Mode: `{report['mode']}`")
    lines.append(f"- Selection: `{selection['kind']}`")
    if selection["kind"] == "suite":
        lines.append(f"- Corpus: `{selection['corpus_path']}`")
        lines.append(f"- Suite: `{selection['suite']}`")
    else:
        lines.append(f"- Filter: \"{selection['filter']}\"")
    lines.append(
        f"- Sample size: {report['method']['sample_size']}, "
        f"warm-up: {report['method']['warm_up_seconds']}s, "
        f"measurement: {report['method']['measurement_seconds']}s"
    )
    lines.append(
        "- Native Criterion targets are built once with `cargo bench --no-run`; "
        "the digest-verified executable is then invoked directly for discovery and timing."
    )
    lines.append("- `merman`: `pipeline --bench ... --exact <benchmark>`")
    lines.append(
        "- `mermaid-rs-renderer` (mmdr): "
        "`renderer --bench ... --exact <benchmark>`"
    )
    lines.append(
        "- Merman/mmdr ratios require byte-identical fixture inputs; non-identical rows retain "
        "their raw timings and measured coverage but are excluded from ratio and geomean aggregates."
    )
    lines.append(
        "- Native values are Criterion console mid-point estimates; native raw samples are not retained."
    )
    lines.append(
        "- `mermaid-js`: warm `mermaid.render()` calls in one Puppeteer/Chromium process; "
        "raw per-call samples and p95/p99 are retained in JSON."
    )
    lines.append(
        "- Native Merman / browser Mermaid.js ratios are diagnostic context only, not a "
        "cross-transport performance ranking."
    )
    lines.append("")
    lines.append("## Coverage Summary")
    lines.append("")
    lines.append("| runner | requested | available | measured | missing | errors | skipped |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for key in ("merman", "mermaid_rs_renderer", "mermaid_js"):
        runner = report["runners"][key]
        cov = runner["coverage"]
        lines.append(
            f"| {runner['label']} | {cov['requested']} | {cov['available']} | "
            f"{cov['measured']} | {cov['missing']} | {cov['errors']} | {cov['skipped']} |"
        )
    lines.append("")
    family_coverage = report["family_coverage"]
    lines.append("## Family Coverage")
    lines.append("")
    lines.append(f"- Requested families: {family_coverage['requested_count']}")
    lines.append(
        "- Measured families: "
        f"Merman {family_coverage['measured_count']['merman']}, "
        f"mmdr {family_coverage['measured_count']['mermaid_rs_renderer']}, "
        f"Mermaid.js {family_coverage['measured_count']['mermaid_js']}"
    )
    lines.append(
        "- Native same-byte comparable families: "
        f"{family_coverage['native_same_byte_comparable_count']}"
    )
    if family_coverage["native_not_same_byte_comparable"]:
        lines.append(
            "- Not in the native same-byte comparable set: "
            + ", ".join(
                f"`{family}`"
                for family in family_coverage["native_not_same_byte_comparable"]
            )
        )
    lines.append("")
    fixture_inputs = report.get("fixture_inputs", {})
    if fixture_inputs:
        input_counts: dict[str, int] = {}
        for comparison in fixture_inputs.values():
            status = str(comparison.get("status") or "unknown")
            input_counts[status] = input_counts.get(status, 0) + 1
        lines.append("## Input Comparability")
        lines.append("")
        lines.append(
            "- "
            + ", ".join(
                f"`{status}`: {count}" for status, count in sorted(input_counts.items())
            )
        )
        non_identical = [
            f"`{name}` ({comparison.get('status', 'unknown')})"
            for name, comparison in fixture_inputs.items()
            if comparison.get("status") != "identical"
        ]
        if non_identical:
            lines.append("- Excluded from Merman/mmdr ratios: " + ", ".join(non_identical) + ".")
        lines.append("")
    lines.append("## Results")
    lines.append("")
    lines.append(
        "| benchmark | family | merman | mermaid-rs-renderer | mermaid-js p50 | "
        "mermaid-js p95 | JS samples | ratio (merman / mmdr) | "
        "context (native merman / browser mermaid-js) |"
    )
    lines.append("|---|---|---:|---:|---:|---:|---:|---:|---:|")
    if report["rows"]:
        for row in report["rows"]:
            lines.append(
                f"| `{row['benchmark']}` | {row['family']} | {fmt_cell(row, 'merman')} | "
                f"{fmt_cell(row, 'mermaid_rs_renderer')} | {fmt_cell(row, 'mermaid_js')} | "
                f"{fmt_js_stat(row, 'p95')} | {fmt_js_stat(row, 'count')} | "
                f"{fmt_mmdr_ratio(row)} | "
                f"{fmt_ratio(row['ratios']['merman_over_mermaid_js'])} |"
            )
    else:
        lines.append("| (no matches) | - | - | - | - | - | - | - | - |")
    lines.append("")

    if report.get("family_summary"):
        lines.append("## Family Summary")
        lines.append("")
        lines.append(
            "| family | fixtures | merman measured | mmdr measured | mermaid-js measured | "
            "geo ratio (merman / mmdr) |"
        )
        lines.append("|---|---:|---:|---:|---:|---:|")
        for row in report["family_summary"]:
            measured = row["measured"]
            ratios = row["geomean_ratios"]
            lines.append(
                f"| {row['family']} | {row['fixtures']} | {measured['merman']} | "
                f"{measured['mermaid_rs_renderer']} | {measured['mermaid_js']} | "
                f"{fmt_ratio(ratios['merman_over_mermaid_rs_renderer'])} |"
            )
        lines.append("")

    lines.append("## Quality and Coverage Caveat")
    lines.append("")
    lines.append(
        "- Timings include only successful renders for each runner. Missing or errored fixtures reduce coverage; they are not folded into ratios."
    )
    lines.append(
        "- Merman/mmdr ratios and family geomean columns include only byte-identical fixture inputs."
    )
    lines.append(
        "- No Mermaid.js family geomean is emitted because native and browser transports are different lanes."
    )
    lines.append(
        "- `merman` is parity-focused and should still be paired with SVG DOM/resvg comparison gates before using performance numbers as a release signal."
    )
    lines.append(
        "- `mermaid-rs-renderer` has different goals and coverage. A faster partial renderer is not equivalent to a parity-compatible renderer."
    )
    lines.append(
        "- The corpus records expected quality gates per fixture; this harness currently records those expectations but does not run DOM or raster comparisons."
    )
    lines.append("")

    for key in ("merman", "mermaid_rs_renderer", "mermaid_js"):
        runner = report["runners"][key]
        missing = runner.get("missing") or []
        errors = runner.get("errors") or {}
        if not missing and not errors:
            continue
        lines.append(f"## Availability: {runner['label']}")
        lines.append("")
        if missing:
            lines.append("Missing:")
            lines.append("")
            lines.append(", ".join(f"`{x}`" for x in missing))
            lines.append("")
        if errors:
            lines.append("Errors:")
            lines.append("")
            for bench, message in sorted(errors.items()):
                first_line = str(message).splitlines()[0] if str(message).splitlines() else str(message)
                lines.append(f"- `{bench}`: {first_line}")
            lines.append("")

    out_path.write_text("\n".join(lines), encoding="utf-8")


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--preset",
        choices=["quick", "long"],
        default="quick",
        help=(
            "Benchmark parameter preset. 'quick' keeps fast iteration defaults; "
            "'long' uses longer measurement to reduce noise."
        ),
    )
    ap.add_argument(
        "--mode",
        choices=["warm"],
        default="warm",
        help="Benchmark mode. Only warm steady-state runs are implemented today.",
    )
    ap.add_argument(
        "--corpus",
        default=DEFAULT_CORPUS,
        help=f"Corpus manifest path (default: {DEFAULT_CORPUS}).",
    )
    ap.add_argument(
        "--suite",
        default="quick",
        help="Corpus suite to run when --filter is not set (default: quick).",
    )
    ap.add_argument(
        "--group",
        default=None,
        help="Criterion group used for suite-driven runs (default: corpus default_group).",
    )
    ap.add_argument(
        "--list-suites",
        action="store_true",
        help="Print corpus suites and exit.",
    )
    ap.add_argument(
        "--mmdr-dir",
        default="repo-ref/mermaid-rs-renderer",
        help="Path to a local checkout of mermaid-rs-renderer (default: repo-ref/mermaid-rs-renderer).",
    )
    ap.add_argument(
        "--mmdr-toolchain",
        default=None,
        help="Optional rustup toolchain for mermaid-rs-renderer cargo commands (e.g. 1.92.0).",
    )
    ap.add_argument(
        "--expected-merman-rev",
        default=None,
        help="Fail before sampling unless Merman resolves to this Git revision.",
    )
    ap.add_argument(
        "--expected-mmdr-rev",
        default=None,
        help="Fail before sampling unless mermaid-rs-renderer resolves to this Git revision.",
    )
    ap.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow dirty checkouts for diagnostic evidence; content is fingerprinted and rechecked.",
    )
    ap.add_argument(
        "--mermaid-cli-dir",
        default="tools/mermaid-cli",
        help="Path to the local Node toolchain used for upstream Mermaid rendering (default: tools/mermaid-cli).",
    )
    ap.add_argument(
        "--out",
        default=DEFAULT_MARKDOWN_OUT,
        help=f"Where to write the Markdown report (default: {DEFAULT_MARKDOWN_OUT}).",
    )
    ap.add_argument(
        "--json-out",
        default=DEFAULT_JSON_OUT,
        help=f"Where to write the structured JSON report (default: {DEFAULT_JSON_OUT}).",
    )
    ap.add_argument(
        "--filter",
        default=None,
        help=(
            "Legacy Criterion filter. When set, --suite is ignored. "
            f"The historical quick filter was: {DEFAULT_QUICK_FILTER}"
        ),
    )
    ap.add_argument("--sample-size", type=int, default=20)
    ap.add_argument("--warm-up", type=int, default=1)
    ap.add_argument("--measurement", type=int, default=1)
    ap.add_argument(
        "--skip-mermaid-js",
        action="store_true",
        help="Skip upstream Mermaid JS benchmarking via puppeteer.",
    )
    args = ap.parse_args(argv)

    def argv_has(opt: str) -> bool:
        return any(a == opt or a.startswith(opt + "=") for a in argv)

    if args.preset == "long":
        if not argv_has("--sample-size"):
            args.sample_size = 30
        if not argv_has("--warm-up"):
            args.warm_up = 2
        if not argv_has("--measurement"):
            args.measurement = 3

    repo_root = Path(__file__).resolve().parents[2]
    corpus_path = (repo_root / args.corpus).resolve()
    corpus = load_corpus(corpus_path)

    if args.list_suites:
        for name, description in sorted(corpus.suites.items()):
            print(f"{name}: {description}")
        return 0

    if args.filter:
        exact_benches = expand_filter_to_exact_benches(args.filter)
        selection = {
            "kind": "filter",
            "filter": args.filter,
            "corpus_path": str(corpus_path.relative_to(repo_root)),
            "suite": None,
        }
    else:
        group = args.group or corpus.default_group
        fixtures = select_corpus_fixtures(corpus, args.suite)
        exact_benches = [f"{group}/{f.name}" for f in fixtures]
        selection = {
            "kind": "suite",
            "filter": None,
            "corpus_path": str(corpus_path.relative_to(repo_root)),
            "suite": args.suite,
            "group": group,
        }

    if not exact_benches:
        raise SystemExit("no benchmark fixtures selected")

    mmdr_dir = (repo_root / args.mmdr_dir).resolve()
    mermaid_cli_dir = (repo_root / args.mermaid_cli_dir).resolve()
    out_path = (repo_root / args.out).resolve()
    json_out_path = (repo_root / args.json_out).resolve()
    mmdr_bench_env = {"MMDR_RUN_CRITERION_BENCHES": "1"}

    if out_path == json_out_path:
        print(
            "[bench][contract] --out and --json-out must resolve to different files",
            file=sys.stderr,
        )
        return 2

    if not mmdr_dir.exists():
        raise SystemExit(
            f"missing mermaid-rs-renderer checkout: {mmdr_dir}\n"
            "expected a local clone at that path (no submodules)."
        )

    fixtures_by_name = {fixture.name: fixture for fixture in corpus.fixtures}
    fixture_names = [split_exact_bench(bench)[1] for bench in exact_benches]
    fixture_inputs = compare_mmdr_fixture_inputs(
        repo_root=repo_root,
        mmdr_dir=mmdr_dir,
        fixture_names=fixture_names,
        fixtures_by_name=fixtures_by_name,
    )
    provenance_files = {
        "benchmark_runner": Path(__file__),
        "corpus": corpus_path,
        "merman_workspace_manifest": repo_root / "Cargo.toml",
        "merman_package_manifest": repo_root / "crates" / "merman" / "Cargo.toml",
        "merman_lockfile": repo_root / "Cargo.lock",
        "merman_pipeline_bench": repo_root / "crates" / "merman" / "benches" / "pipeline.rs",
        "mmdr_manifest": mmdr_dir / "Cargo.toml",
        "mmdr_lockfile": mmdr_dir / "Cargo.lock",
        "mmdr_renderer_bench": mmdr_dir / "benches" / "renderer.rs",
        "mermaid_js_runner": repo_root / "tools" / "bench" / "mermaid_js_bench.cjs",
        "mermaid_cli_lockfile": mermaid_cli_dir / "package-lock.json",
        "mermaid_config": mermaid_cli_dir / "mermaid-config.json",
        "mermaid_bundle": mermaid_cli_dir / "node_modules" / "mermaid" / "dist" / "mermaid.js",
        "mermaid_cli_html": mermaid_cli_dir
        / "node_modules"
        / "@mermaid-js"
        / "mermaid-cli"
        / "dist"
        / "index.html",
        "mermaid_zenuml_bundle": mermaid_cli_dir
        / "node_modules"
        / "@mermaid-js"
        / "mermaid-zenuml"
        / "dist"
        / "mermaid-zenuml.js",
    }
    for output_label, output_path in (("--out", out_path), ("--json-out", json_out_path)):
        for input_label, input_path in provenance_files.items():
            if output_path == input_path.resolve():
                print(
                    f"[bench][contract] {output_label} would overwrite {input_label}",
                    file=sys.stderr,
                )
                return 2
    try:
        repositories_before = {
            "merman": capture_git_provenance(
                repo_root,
                allow_dirty=args.allow_dirty,
                expected_revision=args.expected_merman_rev,
            ),
            "mermaid_rs_renderer": capture_git_provenance(
                mmdr_dir,
                allow_dirty=args.allow_dirty,
                expected_revision=args.expected_mmdr_rev,
            ),
        }
    except ValueError as error:
        print(f"[bench][contract] {error}", file=sys.stderr)
        return 2
    before = ComparisonSnapshot(
        repositories=repositories_before,
        files=snapshot_files(provenance_files),
        fixture_inputs=fixture_inputs,
    )

    merman_prepared = prepare_criterion_runner(
        label="merman",
        cwd=repo_root,
        bench_bin="pipeline",
        package="merman",
        features="svg",
        toolchain=None,
    )
    mmdr_prepared = prepare_criterion_runner(
        label="mermaid-rs-renderer",
        cwd=mmdr_dir,
        bench_bin="renderer",
        package=None,
        features="benchmark",
        env=mmdr_bench_env,
        toolchain=args.mmdr_toolchain,
    )
    merman_list = list_criterion_benches(
        cwd=repo_root,
        runner=merman_prepared,
    )
    mmdr_list = list_criterion_benches(
        cwd=mmdr_dir,
        runner=mmdr_prepared,
        env=mmdr_bench_env,
    )

    merman = run_native_runner(
        label="merman",
        cwd=repo_root,
        runner=merman_prepared,
        exact_benches=exact_benches,
        bench_list=merman_list,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
    )
    mmdr = run_native_runner(
        label="mermaid-rs-renderer",
        cwd=mmdr_dir,
        runner=mmdr_prepared,
        exact_benches=exact_benches,
        bench_list=mmdr_list,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
        env=mmdr_bench_env,
    )

    mermaid_js = run_mermaid_js(
        repo_root=repo_root,
        mermaid_cli_dir=mermaid_cli_dir,
        exact_benches=exact_benches,
        fixtures_by_name=fixtures_by_name,
        sample_warm_up=args.warm_up,
        sample_measurement=args.measurement,
        skip=args.skip_mermaid_js,
    )

    merman["revision"] = before.repositories["merman"]["revision"]
    mmdr["revision"] = before.repositories["mermaid_rs_renderer"]["revision"]

    for runner in (merman, mmdr, mermaid_js):
        runner["coverage"] = coverage_for_runner(runner, exact_benches)

    rows = build_rows(
        exact_benches=exact_benches,
        fixtures_by_name=fixtures_by_name,
        merman=merman,
        mmdr=mmdr,
        mermaid_js=mermaid_js,
        fixture_inputs=fixture_inputs,
    )

    provenance_errors: list[str] = []
    try:
        repositories_after = {
            "merman": capture_git_provenance(
                repo_root,
                allow_dirty=True,
                expected_revision=None,
            ),
            "mermaid_rs_renderer": capture_git_provenance(
                mmdr_dir,
                allow_dirty=True,
                expected_revision=None,
            ),
        }
    except ValueError as error:
        repositories_after = {}
        provenance_errors.append(f"post-sampling Git provenance failed: {error}")
    files_after = snapshot_files(provenance_files)
    try:
        fixture_inputs_after = compare_mmdr_fixture_inputs(
            repo_root=repo_root,
            mmdr_dir=mmdr_dir,
            fixture_names=fixture_names,
            fixtures_by_name=fixtures_by_name,
        )
    except Exception as error:
        fixture_inputs_after = {}
        provenance_errors.append(
            f"post-sampling fixture verification failed: {short_error(error)}"
        )
    after = ComparisonSnapshot(
        repositories=repositories_after,
        files=files_after,
        fixture_inputs=fixture_inputs_after,
    )
    provenance_errors.extend(
        provenance_verification_errors(before=before, after=after)
    )
    for native, prepared in (
        (merman, merman_prepared),
        (mmdr, mmdr_prepared),
    ):
        try:
            verify_criterion_executable(prepared)
        except Exception as error:
            native["executable"]["status"] = "failed"
            native["errors"]["__runner__"] = short_error(error)
    expected_mermaid = locked_mermaid_version(mermaid_cli_dir / "package-lock.json")
    if (
        not args.skip_mermaid_js
        and expected_mermaid is not None
        and mermaid_js.get("revision") != f"mermaid@{expected_mermaid}"
    ):
        provenance_errors.append(
            "Mermaid JS runtime version differs from the package-lock version"
        )
    contract_errors = comparison_contract_errors(
        merman=merman,
        mmdr=mmdr,
        mermaid_js=mermaid_js,
        rows=rows,
        require_mermaid_js=not args.skip_mermaid_js,
        provenance_errors=provenance_errors,
    )

    ts = _dt.datetime.now(_dt.timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M:%S %z")
    family_summary = build_family_summary(rows)
    report: dict[str, Any] = {
        "schema_version": 3,
        "generated_at": ts,
        "mode": args.mode,
        "selection": selection,
        "method": {
            "evidence_class": "diagnostic",
            "admission_eligible": False,
            "sample_size": args.sample_size,
            "warm_up_seconds": args.warm_up,
            "measurement_seconds": args.measurement,
            "criterion_exact_benches": exact_benches,
            "native_estimate_kind": "criterion_console_mid_point",
            "native_raw_samples_retained": False,
            "browser_raw_samples_retained": not args.skip_mermaid_js,
            "cross_transport_ratios_are_context_only": True,
            "command_timeout_seconds": DEFAULT_COMMAND_TIMEOUT_SECONDS,
        },
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "cpu": best_effort_cpu_model(),
            "python": platform.python_version(),
            "rust": rustc_verbose(cwd=repo_root),
            "mmdr_rust": rustc_verbose(
                toolchain=args.mmdr_toolchain,
                cwd=mmdr_dir,
            ),
            "mmdr_toolchain": args.mmdr_toolchain or "default",
        },
        "fixtures": [
            {
                "name": f.name,
                "family": f.family,
                "size": f.size,
                "category": f.category,
                "source": f.source,
                "suites": list(f.suites),
                "features": list(f.features),
                "quality": list(f.quality),
            }
            for f in corpus.fixtures
            if any(f.name == split_exact_bench(b)[1] for b in exact_benches)
        ],
        "fixture_inputs": fixture_inputs,
        "runners": {
            "merman": merman,
            "mermaid_rs_renderer": mmdr,
            "mermaid_js": mermaid_js,
        },
        "rows": rows,
        "family_summary": family_summary,
        "family_coverage": build_family_coverage(family_summary),
        "provenance": {
            "repositories": before.repositories,
            "files": before.files,
            "post_sampling": {
                "repositories": after.repositories,
                "files": after.files,
                "status": "verified" if not provenance_errors else "failed",
                "errors": provenance_errors,
            },
        },
        "contract": {
            "status": "valid_diagnostic" if not contract_errors else "failed",
            "exit_code": 0 if not contract_errors else 2,
            "errors": contract_errors,
            "baseline_eligible": False,
            "baseline_ineligibility_reasons": [
                "native Criterion raw samples are not retained",
                "native and browser transports are not statistically comparable",
                "DOM and raster parity gates are not executed by this runner",
            ],
        },
    }

    write_markdown(out_path, report)
    write_json_report(json_out_path, report)

    print("Wrote:", out_path)
    print("Wrote:", json_out_path)
    if contract_errors:
        for error in contract_errors:
            print(f"[bench][contract] {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

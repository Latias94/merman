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
import json
import math
import os
import platform
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable


DEFAULT_CORPUS = "tools/bench/corpus.json"
DEFAULT_MARKDOWN_OUT = "docs/performance/COMPARISON.md"
DEFAULT_JSON_OUT = "target/bench/renderer_comparison.json"
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
class CorpusFixture:
    name: str
    family: str
    size: str
    category: str
    source: str
    suites: tuple[str, ...]
    features: tuple[str, ...]
    quality: tuple[str, ...]


@dataclass(frozen=True)
class Corpus:
    schema_version: int
    default_group: str
    suites: dict[str, str]
    fixtures: tuple[CorpusFixture, ...]


@dataclass(frozen=True)
class CriterionBenchList:
    benches: set[str]
    skipped: dict[str, list[str]]


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
    if v < 0.1:
        return f"{v:.2f}x"
    return f"{v:.1f}x"


def run(cmd: list[str], cwd: Path, *, env: dict[str, str] | None = None) -> str:
    proc_env = os.environ.copy()
    if env:
        proc_env.update(env)
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
    )
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


def rustc_verbose() -> str:
    try:
        out = subprocess.run(
            ["rustc", "-Vv"],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            check=False,
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


def list_criterion_benches(
    *,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
    env: dict[str, str] | None = None,
    toolchain: str | None = None,
) -> CriterionBenchList:
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
    cmd.extend(["--bench", bench_bin, "--", "--list"])
    out = run(cmd, cwd=cwd, env=env)
    benches: set[str] = set()
    for raw in out.splitlines():
        line = strip_ansi(raw).strip()
        m = _LIST_LINE.match(line)
        if not m:
            continue
        benches.add(m.group("bench"))
    return CriterionBenchList(benches=benches, skipped=parse_skip_lines(out))


def load_corpus(path: Path) -> Corpus:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError(f"corpus must be a JSON object: {path}")
    schema_version = int(data.get("schema_version", 0))
    if schema_version != 1:
        raise ValueError(f"unsupported corpus schema_version: {schema_version}")

    suites_raw = data.get("suites") or {}
    if not isinstance(suites_raw, dict):
        raise ValueError("corpus.suites must be an object")
    suites = {str(k): str(v) for k, v in suites_raw.items()}
    suites.setdefault("full", "All fixtures in corpus order.")

    fixtures_raw = data.get("fixtures") or []
    if not isinstance(fixtures_raw, list):
        raise ValueError("corpus.fixtures must be a list")

    seen: set[str] = set()
    fixtures: list[CorpusFixture] = []
    for idx, item in enumerate(fixtures_raw):
        if not isinstance(item, dict):
            raise ValueError(f"fixture entry {idx} must be an object")
        name = str(item.get("name") or "").strip()
        if not name:
            raise ValueError(f"fixture entry {idx} is missing name")
        if name in seen:
            raise ValueError(f"duplicate fixture in corpus: {name}")
        seen.add(name)

        def str_tuple(key: str) -> tuple[str, ...]:
            value = item.get(key) or []
            if isinstance(value, str):
                return (value,)
            if not isinstance(value, list):
                raise ValueError(f"fixture {name}.{key} must be a string or list")
            return tuple(str(v) for v in value)

        fixtures.append(
            CorpusFixture(
                name=name,
                family=str(item.get("family") or "unknown"),
                size=str(item.get("size") or "unknown"),
                category=str(item.get("category") or "standard"),
                source=str(item.get("source") or f"crates/merman/benches/fixtures/{name}.mmd"),
                suites=str_tuple("suites"),
                features=str_tuple("features"),
                quality=str_tuple("quality"),
            )
        )

    return Corpus(
        schema_version=schema_version,
        default_group=str(data.get("default_group") or "end_to_end"),
        suites=suites,
        fixtures=tuple(fixtures),
    )


def select_corpus_fixtures(corpus: Corpus, suite: str) -> list[CorpusFixture]:
    if suite == "full":
        return list(corpus.fixtures)
    fixtures = [f for f in corpus.fixtures if suite in f.suites]
    if not fixtures:
        available = ", ".join(sorted(corpus.suites))
        raise ValueError(f"unknown or empty suite {suite!r}; available suites: {available}")
    return fixtures


def split_exact_bench(exact: str) -> tuple[str, str]:
    if "/" not in exact:
        return "", exact
    return exact.split("/", 1)


def read_fixture_source(repo_root: Path, name: str, fixture: CorpusFixture | None) -> str | None:
    candidates: list[Path] = []
    if fixture is not None:
        candidates.append(repo_root / fixture.source)
    candidates.append(repo_root / "crates" / "merman" / "benches" / "fixtures" / f"{name}.mmd")
    for path in candidates:
        if path.exists():
            return path.read_text(encoding="utf-8")
    return None


def bench_exact(
    *,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
    exact: str,
    sample_size: int,
    warm_up: int,
    measurement: int,
    env: dict[str, str] | None = None,
    toolchain: str | None = None,
) -> str:
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
    cmd.extend(["--bench", bench_bin, "--"])
    cmd.extend(
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
        ]
    )
    return run(cmd, cwd=cwd, env=env)


def run_native_runner(
    *,
    label: str,
    cwd: Path,
    bench_bin: str,
    package: str | None,
    features: str | None,
    exact_benches: list[str],
    bench_list: CriterionBenchList,
    sample_size: int,
    warm_up: int,
    measurement: int,
    env: dict[str, str] | None = None,
    toolchain: str | None = None,
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

    for exact in available:
        prefix, name = split_exact_bench(exact)
        print("[bench]", label + ":", f"cargo bench --bench {bench_bin} -- ... --exact {exact}")
        try:
            out = bench_exact(
                cwd=cwd,
                bench_bin=bench_bin,
                package=package,
                features=features,
                exact=exact,
                sample_size=sample_size,
                warm_up=warm_up,
                measurement=measurement,
                env=env,
                toolchain=toolchain,
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
    return {
        "label": label,
        "kind": "criterion",
        "available": available,
        "missing": missing,
        "errors": errors,
        "skipped": skipped,
        "times_ns": times_ns,
    }


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
        return {
            "label": "Mermaid JS",
            "kind": "browser_warm",
            "available": [],
            "missing": [],
            "errors": {},
            "skipped": {"end_to_end": end_to_end_names},
            "times_ns": {},
            "samples": {},
            "meta": {},
            "revision": None,
            "skip_reason": "--skip-mermaid-js",
        }
    if not end_to_end_names:
        print("[bench] mermaid-js: skipped (no end_to_end fixtures requested)")
        return {
            "label": "Mermaid JS",
            "kind": "browser_warm",
            "available": [],
            "missing": [],
            "errors": {},
            "skipped": {},
            "times_ns": {},
            "samples": {},
            "meta": {},
            "revision": None,
            "skip_reason": "no end_to_end fixtures requested",
        }
    if not mermaid_cli_dir.exists():
        print("[bench] mermaid-js: skipped (missing tools/mermaid-cli)")
        return {
            "label": "Mermaid JS",
            "kind": "browser_warm",
            "available": [],
            "missing": [],
            "errors": {},
            "skipped": {"end_to_end": end_to_end_names},
            "times_ns": {},
            "samples": {},
            "meta": {},
            "revision": None,
            "skip_reason": f"missing {mermaid_cli_dir}",
        }

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
        return {
            "label": "Mermaid JS",
            "kind": "browser_warm",
            "available": [],
            "missing": missing,
            "errors": {},
            "skipped": {},
            "times_ns": {},
            "samples": {},
            "meta": {},
            "revision": None,
            "skip_reason": "no readable fixtures",
        }

    bench_in = repo_root / "target" / "bench" / "mermaid_js_input.json"
    bench_out = repo_root / "target" / "bench" / "mermaid_js_output.json"
    bench_in.parent.mkdir(parents=True, exist_ok=True)
    bench_in.write_text(
        json.dumps(
            {
                "fixtures": fixtures,
                "configPath": "mermaid-config.json",
                "theme": "default",
                "seed": "1",
                "width": 800,
                "warmupMs": sample_warm_up * 1000,
                "measureMs": sample_measurement * 1000,
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    script = repo_root / "tools" / "bench" / "mermaid_js_bench.cjs"
    print("[bench] mermaid-js (puppeteer): node", script)
    runner_error: str | None = None
    try:
        run(["node", str(script), "--in", str(bench_in), "--out", str(bench_out)], cwd=mermaid_cli_dir)
    except Exception as e:
        runner_error = short_error(e)

    mermaid_js_meta: dict[str, str] = {}
    times_ns: dict[str, float] = {}
    samples: dict[str, int] = {}
    errors: dict[str, str] = {}
    revision: str | None = None

    if runner_error is not None:
        errors["__runner__"] = runner_error
    elif bench_out.exists():
        data = json.loads(bench_out.read_text(encoding="utf-8", errors="replace"))
        if isinstance(data.get("meta"), dict):
            mermaid_js_meta = {
                k: str(v) for k, v in data.get("meta").items() if isinstance(k, str)
            }
        for name, v in (data.get("results") or {}).items():
            if not isinstance(v, dict):
                continue
            med = v.get("median_ns")
            if isinstance(v.get("samples"), int):
                samples[name] = int(v["samples"])
            exact = f"end_to_end/{name}"
            if isinstance(med, (int, float)) and med > 0:
                times_ns[exact] = float(med)
            elif v.get("error"):
                errors[exact] = str(v.get("error"))

        if mermaid_js_meta.get("mermaid"):
            revision = "mermaid@" + mermaid_js_meta["mermaid"]
        else:
            lock = mermaid_cli_dir / "package-lock.json"
            if lock.exists():
                try:
                    lock_data = json.loads(lock.read_text(encoding="utf-8", errors="replace"))
                    ver = (
                        (lock_data.get("packages") or {})
                        .get("node_modules/mermaid", {})
                        .get("version")
                    )
                    if isinstance(ver, str) and ver.strip():
                        revision = "mermaid@" + ver.strip()
                except Exception:
                    revision = None

    return {
        "label": "Mermaid JS",
        "kind": "browser_warm",
        "available": [f"end_to_end/{name}" for name in fixtures],
        "missing": missing,
        "errors": errors,
        "skipped": {},
        "times_ns": times_ns,
        "samples": samples,
        "meta": mermaid_js_meta,
        "revision": revision,
        "skip_reason": None,
    }


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
) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for exact in exact_benches:
        _, name = split_exact_bench(exact)
        fixture = fixtures_by_name.get(name)
        merman_ns = merman.get("times_ns", {}).get(exact)
        mmdr_ns = mmdr.get("times_ns", {}).get(exact)
        mermaid_js_ns = mermaid_js.get("times_ns", {}).get(exact)
        ratio_mmdr = (
            float(merman_ns) / float(mmdr_ns)
            if isinstance(merman_ns, (int, float)) and isinstance(mmdr_ns, (int, float)) and mmdr_ns
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
                "ratios_js": [],
            },
        )
        item["fixtures"] += 1
        for runner in ("merman", "mermaid_rs_renderer", "mermaid_js"):
            if row.get("status", {}).get(runner) == "measured":
                item["measured"][runner] += 1
        ratio_mmdr = row.get("ratios", {}).get("merman_over_mermaid_rs_renderer")
        ratio_js = row.get("ratios", {}).get("merman_over_mermaid_js")
        if isinstance(ratio_mmdr, (int, float)):
            item["ratios_mmdr"].append(float(ratio_mmdr))
        if isinstance(ratio_js, (int, float)):
            item["ratios_js"].append(float(ratio_js))

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
                    "merman_over_mermaid_js": geomean(item["ratios_js"]),
                },
            }
        )
    return out


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
    lines.append("- `merman`: `cargo bench -p merman --features render --bench pipeline -- ...`")
    lines.append("- `mermaid-rs-renderer` (mmdr): `cargo bench --bench renderer -- ...`")
    lines.append("- `mermaid-js`: warm `mermaid.render()` calls in one Puppeteer/Chromium process.")
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
    lines.append("## Results")
    lines.append("")
    lines.append(
        "| benchmark | family | merman | mermaid-rs-renderer | mermaid-js | "
        "ratio (merman / mmdr) | ratio (merman / mermaid-js) |"
    )
    lines.append("|---|---|---:|---:|---:|---:|---:|")
    if report["rows"]:
        for row in report["rows"]:
            lines.append(
                f"| `{row['benchmark']}` | {row['family']} | {fmt_cell(row, 'merman')} | "
                f"{fmt_cell(row, 'mermaid_rs_renderer')} | {fmt_cell(row, 'mermaid_js')} | "
                f"{fmt_ratio(row['ratios']['merman_over_mermaid_rs_renderer'])} | "
                f"{fmt_ratio(row['ratios']['merman_over_mermaid_js'])} |"
            )
    else:
        lines.append("| (no matches) | - | - | - | - | - | - |")
    lines.append("")

    if report.get("family_summary"):
        lines.append("## Family Summary")
        lines.append("")
        lines.append(
            "| family | fixtures | merman measured | mmdr measured | mermaid-js measured | "
            "geo ratio (merman / mmdr) | geo ratio (merman / mermaid-js) |"
        )
        lines.append("|---|---:|---:|---:|---:|---:|---:|")
        for row in report["family_summary"]:
            measured = row["measured"]
            ratios = row["geomean_ratios"]
            lines.append(
                f"| {row['family']} | {row['fixtures']} | {measured['merman']} | "
                f"{measured['mermaid_rs_renderer']} | {measured['mermaid_js']} | "
                f"{fmt_ratio(ratios['merman_over_mermaid_rs_renderer'])} | "
                f"{fmt_ratio(ratios['merman_over_mermaid_js'])} |"
            )
        lines.append("")

    lines.append("## Quality and Coverage Caveat")
    lines.append("")
    lines.append(
        "- Timings include only successful renders for each runner. Missing or errored fixtures reduce coverage; they are not folded into ratios."
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

    if not mmdr_dir.exists():
        raise SystemExit(
            f"missing mermaid-rs-renderer checkout: {mmdr_dir}\n"
            "expected a local clone at that path (no submodules)."
        )

    merman_list = list_criterion_benches(
        cwd=repo_root,
        bench_bin="pipeline",
        package="merman",
        features="render",
        toolchain=None,
    )
    mmdr_list = list_criterion_benches(
        cwd=mmdr_dir,
        bench_bin="renderer",
        package=None,
        features=None,
        env=mmdr_bench_env,
        toolchain=args.mmdr_toolchain,
    )

    merman = run_native_runner(
        label="merman",
        cwd=repo_root,
        bench_bin="pipeline",
        package="merman",
        features="render",
        exact_benches=exact_benches,
        bench_list=merman_list,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
        toolchain=None,
    )
    mmdr = run_native_runner(
        label="mermaid-rs-renderer",
        cwd=mmdr_dir,
        bench_bin="renderer",
        package=None,
        features=None,
        exact_benches=exact_benches,
        bench_list=mmdr_list,
        sample_size=args.sample_size,
        warm_up=args.warm_up,
        measurement=args.measurement,
        env=mmdr_bench_env,
        toolchain=args.mmdr_toolchain,
    )

    fixtures_by_name = {f.name: f for f in corpus.fixtures}
    mermaid_js = run_mermaid_js(
        repo_root=repo_root,
        mermaid_cli_dir=mermaid_cli_dir,
        exact_benches=exact_benches,
        fixtures_by_name=fixtures_by_name,
        sample_warm_up=args.warm_up,
        sample_measurement=args.measurement,
        skip=args.skip_mermaid_js,
    )

    merman["revision"] = git_head(repo_root)
    mmdr["revision"] = git_head(mmdr_dir)

    for runner in (merman, mmdr, mermaid_js):
        runner["coverage"] = coverage_for_runner(runner, exact_benches)

    rows = build_rows(
        exact_benches=exact_benches,
        fixtures_by_name=fixtures_by_name,
        merman=merman,
        mmdr=mmdr,
        mermaid_js=mermaid_js,
    )

    ts = _dt.datetime.now(_dt.timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M:%S %z")
    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at": ts,
        "mode": args.mode,
        "selection": selection,
        "method": {
            "sample_size": args.sample_size,
            "warm_up_seconds": args.warm_up,
            "measurement_seconds": args.measurement,
            "criterion_exact_benches": exact_benches,
        },
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "cpu": best_effort_cpu_model(),
            "python": platform.python_version(),
            "rust": rustc_verbose(),
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
        "runners": {
            "merman": merman,
            "mermaid_rs_renderer": mmdr,
            "mermaid_js": mermaid_js,
        },
        "rows": rows,
        "family_summary": build_family_summary(rows),
    }

    write_markdown(out_path, report)
    write_json_report(json_out_path, report)

    print("Wrote:", out_path)
    print("Wrote:", json_out_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

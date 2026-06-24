#!/usr/bin/env python3
"""
Compare `merman` Criterion benchmarks between two checkouts.

This is the CI-friendly regression gate: it compares the current branch against a base checkout on
the same runner. Cross-repo comparisons remain in `compare_mermaid_renderers.py` because they answer
a different question.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import math
import os
import platform
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from compare_mermaid_renderers import (
    best_effort_cpu_model,
    expand_filter_to_exact_benches,
    git_head,
    list_criterion_benches,
    pretty_time,
    run_native_runner,
    rustc_verbose,
    split_exact_bench,
)
from corpus_utils import load_corpus, select_corpus_fixtures


DEFAULT_CORPUS = "tools/bench/corpus.json"
DEFAULT_MARKDOWN_OUT = "target/bench/self_comparison.md"
DEFAULT_JSON_OUT = "target/bench/self_comparison.json"


@dataclass(frozen=True)
class ComparisonRow:
    benchmark: str
    family: str
    base_ns: float | None
    head_ns: float | None
    change_percent: float | None
    status: str
    reason: str


def benchmark_params(preset: str, sample_size: int | None, warm_up: int | None, measurement: int | None) -> tuple[int, int, int]:
    if preset == "long":
        return (
            sample_size if sample_size is not None else 30,
            warm_up if warm_up is not None else 2,
            measurement if measurement is not None else 3,
        )
    return (
        sample_size if sample_size is not None else 10,
        warm_up if warm_up is not None else 1,
        measurement if measurement is not None else 1,
    )


def geomean(values: Iterable[float]) -> float | None:
    vals = [v for v in values if v > 0.0 and math.isfinite(v)]
    if not vals:
        return None
    return math.exp(sum(math.log(v) for v in vals) / len(vals))


def status_for_native_runner(runner: dict[str, Any], exact: str, name: str) -> str:
    if exact in runner.get("times_ns", {}):
        return "measured"
    if exact in runner.get("errors", {}):
        return "error"
    if exact in runner.get("missing", []):
        return "missing"
    group, _ = split_exact_bench(exact)
    if name in runner.get("skipped", {}).get(group, []):
        return "skipped"
    return "unavailable"


def classify_rows(
    *,
    exact_benches: list[str],
    fixtures_by_name: dict[str, dict[str, Any]],
    base: dict[str, Any],
    head: dict[str, Any],
    warn_threshold_percent: float,
    fail_threshold_percent: float,
) -> list[ComparisonRow]:
    rows: list[ComparisonRow] = []
    for exact in exact_benches:
        _, name = split_exact_bench(exact)
        fixture = fixtures_by_name.get(name, {})
        family = str(fixture.get("family", "unknown"))
        base_ns = base.get("times_ns", {}).get(exact)
        head_ns = head.get("times_ns", {}).get(exact)
        base_status = status_for_native_runner(base, exact, name)
        head_status = status_for_native_runner(head, exact, name)

        if isinstance(base_ns, (int, float)) and isinstance(head_ns, (int, float)) and base_ns > 0:
            change_percent = (float(head_ns) / float(base_ns) - 1.0) * 100.0
            if change_percent > fail_threshold_percent:
                status = "fail"
            elif change_percent > warn_threshold_percent:
                status = "warn"
            elif change_percent < -warn_threshold_percent:
                status = "improved"
            else:
                status = "ok"
            reason = f"head changed by {change_percent:+.2f}%"
        elif base_status == "measured" and head_status != "measured":
            change_percent = None
            status = "fail"
            reason = f"head benchmark is {head_status}"
        elif head_status == "error":
            change_percent = None
            status = "fail"
            reason = "head benchmark errored"
        else:
            change_percent = None
            status = "skipped"
            reason = f"base={base_status}, head={head_status}"

        rows.append(
            ComparisonRow(
                benchmark=exact,
                family=family,
                base_ns=float(base_ns) if isinstance(base_ns, (int, float)) else None,
                head_ns=float(head_ns) if isinstance(head_ns, (int, float)) else None,
                change_percent=change_percent,
                status=status,
                reason=reason,
            )
        )
    return rows


def coverage_for_runner(runner: dict[str, Any], exact_benches: list[str]) -> dict[str, int]:
    measured = len(runner.get("times_ns", {}))
    missing = len(runner.get("missing", []))
    errors = len(runner.get("errors", {}))
    skipped = 0
    for exact in exact_benches:
        group, name = split_exact_bench(exact)
        if name in runner.get("skipped", {}).get(group, []):
            skipped += 1
    return {
        "requested": len(exact_benches),
        "available": len(runner.get("available", [])),
        "measured": measured,
        "missing": missing,
        "errors": errors,
        "skipped": skipped,
    }


def run_checkout(
    *,
    label: str,
    checkout: Path,
    exact_benches: list[str],
    sample_size: int,
    warm_up: int,
    measurement: int,
    target_dir: Path | None,
) -> dict[str, Any]:
    env = os.environ.copy()
    env["CARGO_INCREMENTAL"] = "0"
    env["CARGO_PROFILE_BENCH_DEBUG"] = "0"
    if target_dir is not None:
        target_dir.mkdir(parents=True, exist_ok=True)
        env["CARGO_TARGET_DIR"] = str(target_dir)

    bench_list = list_criterion_benches(
        cwd=checkout,
        bench_bin="pipeline",
        package="merman",
        features="render",
        env=env,
        toolchain=None,
    )
    runner = run_native_runner(
        label=label,
        cwd=checkout,
        bench_bin="pipeline",
        package="merman",
        features="render",
        exact_benches=exact_benches,
        bench_list=bench_list,
        sample_size=sample_size,
        warm_up=warm_up,
        measurement=measurement,
        env=env,
        toolchain=None,
    )
    runner["revision"] = git_head(checkout)
    return runner


def write_json_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def fmt_percent(value: float | None) -> str:
    if value is None:
        return "-"
    return f"{value:+.2f}%"


def fmt_time(value: float | None) -> str:
    if value is None:
        return "-"
    return pretty_time(value)


def write_markdown(path: Path, report: dict[str, Any], rows: list[ComparisonRow]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    summary = report["summary"]
    method = report["method"]
    env = report["environment"]

    lines: list[str] = []
    lines.append("# Merman Self Performance Comparison")
    lines.append("")
    lines.append("> Generated by `tools/bench/compare_self.py`.")
    lines.append("")
    lines.append("## Summary")
    lines.append("")
    lines.append(f"- Gate: `{summary['gate_status']}`")
    lines.append(f"- Comparable benchmarks: `{summary['comparable']}`")
    lines.append(f"- Failures: `{summary['failures']}`")
    lines.append(f"- Warnings: `{summary['warnings']}`")
    lines.append(f"- Improvements: `{summary['improvements']}`")
    lines.append(f"- Geomean change: `{fmt_percent(summary['geomean_change_percent'])}`")
    lines.append("")
    lines.append("## Method")
    lines.append("")
    lines.append(f"- Suite: `{report['selection']['suite']}`")
    lines.append(f"- Group: `{report['selection']['group']}`")
    lines.append(f"- Sample size: `{method['sample_size']}`")
    lines.append(f"- Warm-up: `{method['warm_up_seconds']}s`")
    lines.append(f"- Measurement: `{method['measurement_seconds']}s`")
    lines.append(f"- Preset: `{method['preset']}`")
    lines.append(f"- Warn threshold: `+{method['warn_threshold_percent']:.2f}%`")
    lines.append(f"- Fail threshold: `+{method['fail_threshold_percent']:.2f}%`")
    lines.append("")
    lines.append("## Environment")
    lines.append("")
    lines.append(f"- Timestamp: `{report['generated_at']}`")
    lines.append(f"- OS: `{env['os']}`")
    lines.append(f"- Machine: `{env['machine']}`")
    lines.append(f"- CPU: `{env['cpu']}`")
    lines.append(f"- Python: `{env['python']}`")
    lines.append(f"- Base label: `{report['comparison']['base_label']}`")
    lines.append(f"- Head label: `{report['comparison']['head_label']}`")
    lines.append(f"- Base revision: `{report['runners']['base'].get('revision') or 'unknown'}`")
    lines.append(f"- Head revision: `{report['runners']['head'].get('revision') or 'unknown'}`")
    lines.append("")
    lines.append("```")
    lines.append(env["rust"])
    lines.append("```")
    lines.append("")
    lines.append("## Results")
    lines.append("")
    lines.append("| benchmark | family | base | head | change | status | reason |")
    lines.append("|---|---|---:|---:|---:|---|---|")
    for row in rows:
        lines.append(
            f"| `{row.benchmark}` | {row.family} | {fmt_time(row.base_ns)} | "
            f"{fmt_time(row.head_ns)} | {fmt_percent(row.change_percent)} | "
            f"`{row.status}` | {row.reason} |"
        )
    lines.append("")
    lines.append("## Coverage")
    lines.append("")
    lines.append("| runner | requested | available | measured | missing | errors | skipped |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|")
    for key in ("base", "head"):
        runner = report["runners"][key]
        cov = runner["coverage"]
        lines.append(
            f"| {runner['label']} | {cov['requested']} | {cov['available']} | "
            f"{cov['measured']} | {cov['missing']} | {cov['errors']} | {cov['skipped']} |"
        )
    lines.append("")
    lines.append("## Caveat")
    lines.append("")
    lines.append(
        "This is a same-runner regression signal, not an absolute performance guarantee. "
        "Re-run locally or with the long preset before making fine-grained optimization claims."
    )
    lines.append("")
    path.write_text("\n".join(lines), encoding="utf-8")


def build_summary(rows: list[ComparisonRow]) -> dict[str, Any]:
    comparable = [row for row in rows if row.base_ns is not None and row.head_ns is not None]
    ratios = [row.head_ns / row.base_ns for row in comparable if row.base_ns and row.head_ns]
    gmean = geomean(ratios)
    failures = sum(1 for row in rows if row.status == "fail")
    warnings = sum(1 for row in rows if row.status == "warn")
    improvements = sum(1 for row in rows if row.status == "improved")
    no_comparable_failure = 1 if not comparable else 0
    failures += no_comparable_failure
    return {
        "gate_status": "fail" if failures else "pass",
        "comparable": len(comparable),
        "failures": failures,
        "warnings": warnings,
        "improvements": improvements,
        "geomean_change_percent": ((gmean - 1.0) * 100.0) if gmean is not None else None,
        "no_comparable_failure": bool(no_comparable_failure),
    }


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Compare merman Criterion results between two checkouts.")
    ap.add_argument("--base-dir", required=True, help="Base checkout path.")
    ap.add_argument("--head-dir", required=True, help="Head checkout path.")
    ap.add_argument("--base-label", default="base", help="Human-readable base ref label.")
    ap.add_argument("--head-label", default="head", help="Human-readable head ref label.")
    ap.add_argument("--base-target-dir", default="", help="Optional Cargo target dir for base.")
    ap.add_argument("--head-target-dir", default="", help="Optional Cargo target dir for head.")
    ap.add_argument("--corpus", default=DEFAULT_CORPUS, help=f"Corpus path relative to head (default: {DEFAULT_CORPUS}).")
    ap.add_argument("--suite", default="canary", help="Corpus suite to compare (default: canary).")
    ap.add_argument("--group", default=None, help="Criterion group (default: corpus default_group).")
    ap.add_argument("--filter", default=None, help="Exact benchmark filter; overrides --suite.")
    ap.add_argument("--preset", choices=["quick", "long"], default="quick")
    ap.add_argument("--sample-size", type=int, default=None)
    ap.add_argument("--warm-up", type=int, default=None)
    ap.add_argument("--measurement", type=int, default=None)
    ap.add_argument("--warn-threshold-percent", type=float, default=5.0)
    ap.add_argument("--fail-threshold-percent", type=float, default=10.0)
    ap.add_argument("--out", default=DEFAULT_MARKDOWN_OUT)
    ap.add_argument("--json-out", default=DEFAULT_JSON_OUT)
    ap.add_argument("--no-fail", action="store_true", help="Always exit 0 after writing reports.")
    args = ap.parse_args(argv)

    base_dir = Path(args.base_dir).resolve()
    head_dir = Path(args.head_dir).resolve()
    corpus_path = (head_dir / args.corpus).resolve()
    corpus = load_corpus(corpus_path)

    if args.filter:
        exact_benches = expand_filter_to_exact_benches(args.filter)
        selection = {
            "kind": "filter",
            "filter": args.filter,
            "suite": None,
            "group": None,
            "corpus_path": str(corpus_path),
        }
    else:
        group = args.group or corpus.default_group
        selected_fixtures = select_corpus_fixtures(corpus, args.suite)
        exact_benches = [f"{group}/{fixture.name}" for fixture in selected_fixtures]
        selection = {
            "kind": "suite",
            "filter": None,
            "suite": args.suite,
            "group": group,
            "corpus_path": str(corpus_path),
        }

    if not exact_benches:
        raise SystemExit("no benchmark fixtures selected")

    sample_size, warm_up, measurement = benchmark_params(
        args.preset,
        args.sample_size,
        args.warm_up,
        args.measurement,
    )

    base_target = Path(args.base_target_dir).resolve() if args.base_target_dir else None
    head_target = Path(args.head_target_dir).resolve() if args.head_target_dir else None

    base = run_checkout(
        label="base",
        checkout=base_dir,
        exact_benches=exact_benches,
        sample_size=sample_size,
        warm_up=warm_up,
        measurement=measurement,
        target_dir=base_target,
    )
    head = run_checkout(
        label="head",
        checkout=head_dir,
        exact_benches=exact_benches,
        sample_size=sample_size,
        warm_up=warm_up,
        measurement=measurement,
        target_dir=head_target,
    )

    fixtures_by_name = {
        fixture.name: {
            "family": fixture.family,
            "size": fixture.size,
            "category": fixture.category,
            "source": fixture.source,
            "features": list(fixture.features),
            "quality": list(fixture.quality),
        }
        for fixture in corpus.fixtures
    }
    rows = classify_rows(
        exact_benches=exact_benches,
        fixtures_by_name=fixtures_by_name,
        base=base,
        head=head,
        warn_threshold_percent=args.warn_threshold_percent,
        fail_threshold_percent=args.fail_threshold_percent,
    )
    summary = build_summary(rows)

    for runner in (base, head):
        runner["coverage"] = coverage_for_runner(runner, exact_benches)

    generated_at = dt.datetime.now(dt.timezone.utc).astimezone().strftime("%Y-%m-%d %H:%M:%S %z")
    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at": generated_at,
        "comparison": {
            "base_label": args.base_label,
            "head_label": args.head_label,
        },
        "selection": selection,
        "method": {
            "preset": args.preset,
            "sample_size": sample_size,
            "warm_up_seconds": warm_up,
            "measurement_seconds": measurement,
            "warn_threshold_percent": args.warn_threshold_percent,
            "fail_threshold_percent": args.fail_threshold_percent,
            "criterion_exact_benches": exact_benches,
        },
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "cpu": best_effort_cpu_model(),
            "python": platform.python_version(),
            "rust": rustc_verbose(),
        },
        "fixtures": [
            {
                "name": split_exact_bench(exact)[1],
                **fixtures_by_name.get(split_exact_bench(exact)[1], {}),
            }
            for exact in exact_benches
        ],
        "runners": {
            "base": base,
            "head": head,
        },
        "rows": [row.__dict__ for row in rows],
        "summary": summary,
    }

    out_path = (head_dir / args.out).resolve() if not Path(args.out).is_absolute() else Path(args.out)
    json_out_path = (head_dir / args.json_out).resolve() if not Path(args.json_out).is_absolute() else Path(args.json_out)
    write_markdown(out_path, report, rows)
    write_json_report(json_out_path, report)

    print("Wrote:", out_path)
    print("Wrote:", json_out_path)
    print("Gate:", summary["gate_status"])
    if summary["no_comparable_failure"]:
        print("No comparable benchmark measurements were produced.", file=sys.stderr)

    if summary["gate_status"] == "fail" and not args.no_fail:
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

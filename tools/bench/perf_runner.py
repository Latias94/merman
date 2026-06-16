#!/usr/bin/env python3
"""
One-step performance runner for the standard `merman` optimization workflow.

Profiles:
- triage: correctness gate + stage spotcheck
- canary: triage + canary end-to-end comparison
- full: canary + broader suite comparison + stress benches
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import shlex
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from corpus_utils import fixture_names_for_suite, load_corpus


DEFAULT_CORPUS_PATH = Path(__file__).resolve().with_name("corpus.json")
STANDARD_CANARY_FIXTURES = ",".join(
    fixture_names_for_suite(load_corpus(DEFAULT_CORPUS_PATH), "canary")
)
STANDARD_CANARY_SUITE = "canary"
DEFAULT_COMPARE_SUITE = "standard"
STRESS_BENCHES = [
    "flowchart_stress",
    "architecture_layout_stress",
    "architecture_stress",
    "mindmap_layout_stress",
    "text_measure_stress",
]


@dataclass(frozen=True)
class Step:
    label: str
    cmd: list[str]
    cwd: Path
    env: dict[str, str] | None = None


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def today_stamp() -> str:
    return dt.date.today().isoformat()


def python_cmd(root: Path, script: str, extra_args: list[str]) -> list[str]:
    return [sys.executable, str(root / "tools" / "bench" / script), *extra_args]


def cli_path(root: Path, path: Path) -> str:
    try:
        return str(path.relative_to(root))
    except ValueError:
        return str(path)


def stage_bench_params(preset: str) -> tuple[int, int, int]:
    if preset == "long":
        return 30, 2, 3
    return 20, 1, 1


def cargo_bench_cmd(
    *,
    bench: str,
    exact: str | None,
    sample_size: int,
    warm_up: int,
    measurement: int,
    package: str = "merman",
    features: str = "render",
) -> list[str]:
    cmd = ["cargo", "bench", "--locked"]
    if package:
        cmd.extend(["-p", package])
    if features:
        cmd.extend(["--features", features])
    cmd.extend(
        [
            "--bench",
            bench,
            "--",
            "--noplot",
            "--sample-size",
            str(sample_size),
            "--warm-up-time",
            str(warm_up),
            "--measurement-time",
            str(measurement),
            "--discard-baseline",
        ]
    )
    if exact is not None:
        cmd.extend(["--exact", exact])
    return cmd


def render_target_path(
    *,
    report_root: Path,
    docs: bool,
    profile: str,
    kind: str,
    suffix: str,
) -> Path:
    stamp = today_stamp()
    if docs:
        if kind == "spotcheck":
            name = f"spotcheck_{stamp}_perf-runner_{profile}.{suffix}"
        else:
            name = f"COMPARISON.perf-runner_{stamp}_{profile}.{suffix}"
        return repo_root() / "docs" / "performance" / name
    return report_root / f"{stamp}_{profile}_{kind}.{suffix}"


def suite_target_path(
    *,
    report_root: Path,
    docs: bool,
    profile: str,
    suite: str,
    suffix: str,
) -> Path:
    stamp = today_stamp()
    if docs:
        name = f"COMPARISON.perf-runner_{stamp}_{profile}_suite_{suite}.{suffix}"
        return repo_root() / "docs" / "performance" / name
    return report_root / f"{stamp}_{profile}_suite_{suite}.{suffix}"


def build_steps(args: argparse.Namespace) -> list[Step]:
    root = repo_root()
    report_root = (root / args.report_root).resolve() if not Path(args.report_root).is_absolute() else Path(args.report_root)
    docs = args.write_docs

    steps: list[Step] = []

    steps.append(
        Step(
            label="correctness gate",
            cmd=["cargo", "nextest", "run", "-p", "merman-render"],
            cwd=root,
        )
    )

    if args.profile in {"triage", "canary", "full"}:
        stage_out = render_target_path(
            report_root=report_root,
            docs=docs,
            profile=args.profile,
            kind="spotcheck",
            suffix="md",
        )
        stage_cmd = python_cmd(
            root,
            "stage_spotcheck.py",
            [
                "--preset",
                args.preset,
                "--fixtures",
                args.stage_fixtures,
                "--out",
                cli_path(root, stage_out),
                "--mmdr-dir",
                args.mmdr_dir,
            ]
            + (["--mmdr-toolchain", args.mmdr_toolchain] if args.mmdr_toolchain else []),
        )
        steps.append(
            Step(
                label=f"stage spotcheck ({args.stage_fixtures})",
                cmd=stage_cmd,
                cwd=root,
            )
        )

        if args.include_cold_parse:
            sample_size, warm_up, measurement = stage_bench_params(args.preset)
            for fixture in [x.strip() for x in args.cold_parse_fixtures.split(",") if x.strip()]:
                exact = f"parse_cold_engine/{fixture}"
                steps.append(
                    Step(
                        label=f"cold parse ({fixture})",
                        cmd=cargo_bench_cmd(
                            bench="pipeline",
                            exact=exact,
                            sample_size=sample_size,
                            warm_up=warm_up,
                            measurement=measurement,
                        ),
                        cwd=root,
                    )
                )

    if args.profile in {"canary", "full"}:
        compare_out = render_target_path(
            report_root=report_root,
            docs=docs,
            profile=args.profile,
            kind="comparison",
            suffix="md",
        )
        compare_json = render_target_path(
            report_root=report_root,
            docs=False,
            profile=args.profile,
            kind="comparison",
            suffix="json",
        )
        compare_cmd = python_cmd(
            root,
            "compare_mermaid_renderers.py",
            [
                "--preset",
                args.preset,
            ]
            + (
                ["--filter", args.compare_filter]
                if args.compare_filter
                else ["--suite", args.canary_suite]
            )
            + [
                "--out",
                cli_path(root, compare_out),
                "--json-out",
                cli_path(root, compare_json),
                "--mmdr-dir",
                args.mmdr_dir,
            ]
            + (["--mmdr-toolchain", args.mmdr_toolchain] if args.mmdr_toolchain else [])
            + ([] if args.include_mermaid_js else ["--skip-mermaid-js"]),
        )
        steps.append(
            Step(
                label="canary compare vs mmdr",
                cmd=compare_cmd,
                cwd=root,
            )
        )

    if args.profile == "full":
        suite_compare_out = suite_target_path(
            report_root=report_root,
            docs=docs,
            profile=args.profile,
            suite=args.compare_suite,
            suffix="md",
        )
        suite_compare_json = suite_target_path(
            report_root=report_root,
            docs=False,
            profile=args.profile,
            suite=args.compare_suite,
            suffix="json",
        )
        suite_cmd = python_cmd(
            root,
            "compare_mermaid_renderers.py",
            [
                "--preset",
                args.preset,
                "--suite",
                args.compare_suite,
                "--out",
                cli_path(root, suite_compare_out),
                "--json-out",
                cli_path(root, suite_compare_json),
                "--mmdr-dir",
                args.mmdr_dir,
            ]
            + (["--mmdr-toolchain", args.mmdr_toolchain] if args.mmdr_toolchain else [])
            + ([] if args.include_mermaid_js else ["--skip-mermaid-js"]),
        )
        steps.append(
            Step(
                label=f"broader compare suite ({args.compare_suite})",
                cmd=suite_cmd,
                cwd=root,
            )
        )

        for bench_bin in STRESS_BENCHES:
            steps.append(
                Step(
                    label=f"stress bench ({bench_bin})",
                    cmd=cargo_bench_cmd(
                        bench=bench_bin,
                        exact=None,
                        sample_size=args.stress_sample_size,
                        warm_up=args.stress_warm_up,
                        measurement=args.stress_measurement,
                    ),
                    cwd=root,
                )
            )

    return steps


def run_step(step: Step, *, dry_run: bool) -> None:
    print(f"\n==> {step.label}")
    print(f"$ {shlex.join(step.cmd)}")
    if dry_run:
        return

    env = os.environ.copy()
    if step.env:
        env.update(step.env)

    start = time.perf_counter()
    proc = subprocess.run(step.cmd, cwd=str(step.cwd), env=env)
    elapsed = time.perf_counter() - start
    if proc.returncode != 0:
        raise SystemExit(proc.returncode)
    print(f"[ok] {step.label} ({elapsed:.1f}s)")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Run the documented performance workflow in one pass."
    )
    parser.add_argument(
        "--profile",
        choices=["triage", "canary", "full"],
        default="canary",
        help="Workflow preset to run (default: canary).",
    )
    parser.add_argument(
        "--preset",
        choices=["quick", "long"],
        default="long",
        help="Benchmark preset passed to stage/comparison scripts (default: long).",
    )
    parser.add_argument(
        "--stage-fixtures",
        default=STANDARD_CANARY_FIXTURES,
        help=f"Comma-separated fixtures for stage spotcheck (default: {STANDARD_CANARY_FIXTURES}).",
    )
    parser.add_argument(
        "--compare-filter",
        default="",
        help=(
            "Optional exact comparison filter for the canary compare step. "
            "When omitted, the canary suite from corpus.json is used."
        ),
    )
    parser.add_argument(
        "--canary-suite",
        default=STANDARD_CANARY_SUITE,
        help="Corpus suite used for the canary compare step (default: canary).",
    )
    parser.add_argument(
        "--compare-suite",
        default=DEFAULT_COMPARE_SUITE,
        help="Suite used by the broader comparison step in full profile.",
    )
    parser.add_argument(
        "--cold-parse-fixtures",
        default=STANDARD_CANARY_FIXTURES,
        help="Comma-separated fixtures for parse_cold_engine sanity checks.",
    )
    parser.add_argument(
        "--include-cold-parse",
        action="store_true",
        help="Include parse_cold_engine sanity checks after stage attribution.",
    )
    parser.add_argument(
        "--include-mermaid-js",
        action="store_true",
        help="Also run the Mermaid JS comparison path (defaults to skipped).",
    )
    parser.add_argument(
        "--mmdr-dir",
        default="repo-ref/mermaid-rs-renderer",
        help="Path to a local checkout of mermaid-rs-renderer.",
    )
    parser.add_argument(
        "--mmdr-toolchain",
        default=None,
        help="Optional rustup toolchain for mermaid-rs-renderer cargo commands.",
    )
    parser.add_argument(
        "--report-root",
        default="target/bench/perf-runner",
        help="Root directory for local artifacts when not writing to docs.",
    )
    parser.add_argument(
        "--write-docs",
        action="store_true",
        help=(
            "Write perf-runner Markdown reports under docs/performance. "
            "Structured JSON artifacts still use --report-root."
        ),
    )
    parser.add_argument(
        "--stress-sample-size",
        type=int,
        default=50,
        help="Sample size used by the stress benches (default: 50).",
    )
    parser.add_argument(
        "--stress-warm-up",
        type=int,
        default=2,
        help="Warm-up seconds used by the stress benches (default: 2).",
    )
    parser.add_argument(
        "--stress-measurement",
        type=int,
        default=3,
        help="Measurement seconds used by the stress benches (default: 3).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the planned commands without executing them.",
    )
    args = parser.parse_args(argv)

    steps = build_steps(args)

    print(f"Profile: {args.profile}")
    print(f"Preset: {args.preset}")
    if args.write_docs:
        print(f"Output mode: docs/performance (Markdown), {Path(args.report_root).expanduser()} (JSON)")
    else:
        print(f"Output mode: {Path(args.report_root).expanduser()}")

    for step in steps:
        run_step(step, dry_run=args.dry_run)

    if not args.dry_run:
        print("\nArtifacts:")
        root = repo_root()
        if args.profile in {"triage", "canary", "full"}:
            stage_out = render_target_path(
                report_root=(root / args.report_root).resolve() if not Path(args.report_root).is_absolute() else Path(args.report_root),
                docs=args.write_docs,
                profile=args.profile,
                kind="spotcheck",
                suffix="md",
            )
            print(f"- {stage_out}")
        if args.profile in {"canary", "full"}:
            compare_out = render_target_path(
                report_root=(root / args.report_root).resolve() if not Path(args.report_root).is_absolute() else Path(args.report_root),
                docs=args.write_docs,
                profile=args.profile,
                kind="comparison",
                suffix="md",
            )
            print(f"- {compare_out}")
        if args.profile == "full":
            suite_out = suite_target_path(
                report_root=(root / args.report_root).resolve() if not Path(args.report_root).is_absolute() else Path(args.report_root),
                docs=args.write_docs,
                profile=args.profile,
                suite=args.compare_suite,
                suffix="md",
            )
            print(f"- {suite_out}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

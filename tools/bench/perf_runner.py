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
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

import run_native_memory
from corpus_utils import fixture_names_for_suite, load_corpus


DEFAULT_CORPUS_PATH = Path(__file__).resolve().with_name("corpus.json")
STANDARD_CANARY_FIXTURES = ",".join(
    fixture_names_for_suite(load_corpus(DEFAULT_CORPUS_PATH), "canary")
)
STANDARD_CANARY_SUITE = "canary"
DEFAULT_COMPARE_SUITE = "standard"
DEFAULT_NATIVE_MEMORY_LANE = "flowchart-end-to-end-memory"
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


@dataclass(frozen=True)
class ReportPublication:
    source: Path
    destination: Path


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
    features: str = "svg",
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
            name = f"renderer_comparison_{stamp}_perf-runner_{profile}.{suffix}"
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
        name = f"renderer_comparison_{stamp}_perf-runner_{profile}_suite_{suite}.{suffix}"
        return repo_root() / "docs" / "performance" / name
    return report_root / f"{stamp}_{profile}_suite_{suite}.{suffix}"


def native_memory_target_path(*, report_root: Path, profile: str) -> Path:
    return report_root / f"{today_stamp()}_{profile}_native_memory.json"


def resolved_report_root(value: str) -> Path:
    path = Path(value)
    return (repo_root() / path).resolve() if not path.is_absolute() else path


def build_report_publications(args: argparse.Namespace) -> list[ReportPublication]:
    report_root = resolved_report_root(args.report_root)
    publications = [
        ReportPublication(
            source=render_target_path(
                report_root=report_root,
                docs=False,
                profile=args.profile,
                kind="spotcheck",
                suffix="md",
            ),
            destination=render_target_path(
                report_root=report_root,
                docs=True,
                profile=args.profile,
                kind="spotcheck",
                suffix="md",
            ),
        )
    ]
    if args.profile in {"canary", "full"}:
        publications.append(
            ReportPublication(
                source=render_target_path(
                    report_root=report_root,
                    docs=False,
                    profile=args.profile,
                    kind="comparison",
                    suffix="md",
                ),
                destination=render_target_path(
                    report_root=report_root,
                    docs=True,
                    profile=args.profile,
                    kind="comparison",
                    suffix="md",
                ),
            )
        )
    if args.profile == "full":
        publications.append(
            ReportPublication(
                source=suite_target_path(
                    report_root=report_root,
                    docs=False,
                    profile=args.profile,
                    suite=args.compare_suite,
                    suffix="md",
                ),
                destination=suite_target_path(
                    report_root=report_root,
                    docs=True,
                    profile=args.profile,
                    suite=args.compare_suite,
                    suffix="md",
                ),
            )
        )
    return publications


def publish_reports(
    publications: list[ReportPublication], *, dry_run: bool
) -> None:
    print("\n==> publish Markdown reports")
    for publication in publications:
        print(
            f"- {cli_path(repo_root(), publication.source)} -> "
            f"{cli_path(repo_root(), publication.destination)}"
        )
    if dry_run:
        return

    missing = [item.source for item in publications if not item.source.is_file()]
    if missing:
        joined = ", ".join(str(path) for path in missing)
        raise FileNotFoundError(f"performance reports were not produced: {joined}")
    for publication in publications:
        publication.destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(publication.source, publication.destination)


def build_steps(args: argparse.Namespace) -> list[Step]:
    root = repo_root()
    report_root = resolved_report_root(args.report_root)

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
            docs=False,
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
            docs=False,
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
            docs=False,
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

    if args.include_native_memory:
        memory_out = native_memory_target_path(
            report_root=report_root,
            profile=args.profile,
        )
        memory_cmd = python_cmd(
            root,
            "run_native_memory.py",
            [
                "--corpus",
                args.corpus,
                "--lane",
                args.native_memory_lane,
                "--repeats",
                str(args.native_memory_repeats),
                "--seed",
                str(args.native_memory_seed),
                "--bootstrap-resamples",
                str(args.native_memory_bootstrap_resamples),
                "--json-out",
                cli_path(root, memory_out),
            ]
            + (
                ["--contract", args.native_memory_contract]
                if args.native_memory_contract
                else []
            )
            + (
                ["--toolchain", args.native_memory_toolchain]
                if args.native_memory_toolchain
                else []
            ),
        )
        steps.append(
            Step(
                label=f"native memory ({args.native_memory_lane})",
                cmd=memory_cmd,
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
        "--corpus",
        default=str(DEFAULT_CORPUS_PATH.relative_to(repo_root())),
        help="Corpus registry used by opt-in native-memory evidence.",
    )
    parser.add_argument(
        "--include-native-memory",
        action="store_true",
        help="Run the isolated native-memory driver after the selected latency profile.",
    )
    parser.add_argument(
        "--native-memory-lane",
        default=DEFAULT_NATIVE_MEMORY_LANE,
        help="Registered native-memory lane to run.",
    )
    parser.add_argument(
        "--native-memory-contract",
        default="",
        help="Optional owner evidence contract override; defaults to lane metadata.",
    )
    parser.add_argument(
        "--native-memory-toolchain",
        default="",
        help="Optional rustup toolchain used only for the native-memory executable.",
    )
    parser.add_argument(
        "--native-memory-repeats",
        type=int,
        default=5,
        help="Fresh operation/zero pairs at every registered scale (default: 5).",
    )
    parser.add_argument(
        "--native-memory-seed",
        type=int,
        default=run_native_memory.DEFAULT_SEED,
        help="Fixed generator seed shared by the complete memory matrix.",
    )
    parser.add_argument(
        "--native-memory-bootstrap-resamples",
        type=int,
        default=run_native_memory.DEFAULT_BOOTSTRAP_RESAMPLES,
        help="Matched-vector bootstrap resamples for native-memory bounds.",
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
    publications = build_report_publications(args) if args.write_docs else []

    print(f"Profile: {args.profile}")
    print(f"Preset: {args.preset}")
    if args.write_docs:
        print(f"Output mode: docs/performance (Markdown), {Path(args.report_root).expanduser()} (JSON)")
    else:
        print(f"Output mode: {Path(args.report_root).expanduser()}")

    for step in steps:
        run_step(step, dry_run=args.dry_run)

    if publications:
        publish_reports(publications, dry_run=args.dry_run)

    if not args.dry_run:
        print("\nArtifacts:")
        report_root = resolved_report_root(args.report_root)
        published_by_source = {
            publication.source: publication.destination
            for publication in publications
        }
        if args.profile in {"triage", "canary", "full"}:
            stage_out = render_target_path(
                report_root=report_root,
                docs=False,
                profile=args.profile,
                kind="spotcheck",
                suffix="md",
            )
            print(f"- {published_by_source.get(stage_out, stage_out)}")
        if args.profile in {"canary", "full"}:
            compare_out = render_target_path(
                report_root=report_root,
                docs=False,
                profile=args.profile,
                kind="comparison",
                suffix="md",
            )
            print(f"- {published_by_source.get(compare_out, compare_out)}")
        if args.profile == "full":
            suite_out = suite_target_path(
                report_root=report_root,
                docs=False,
                profile=args.profile,
                suite=args.compare_suite,
                suffix="md",
            )
            print(f"- {published_by_source.get(suite_out, suite_out)}")
        if args.include_native_memory:
            memory_out = native_memory_target_path(
                report_root=report_root,
                profile=args.profile,
            )
            print(f"- {memory_out}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

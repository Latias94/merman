#!/usr/bin/env python3
"""Build one Criterion bench and verify its list against lane metadata."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from collections import Counter
from collections.abc import Sequence
from pathlib import Path

from compare_mermaid_renderers import strip_ansi
from compare_self import (
    _NATIVE_CRITERION_PREFLIGHT_CONTRACT,
    RunnerRecipe,
    cargo_prebuild_command,
    criterion_list_command,
    parse_bench_executable,
    parse_preflight_receipts,
)
from corpus_utils import Corpus, lane_selector_group, load_corpus


ROOT = Path(__file__).resolve().parents[2]
_LIST_LINE = re.compile(r"^(?P<bench>[A-Za-z0-9_.:/-]+):\s*benchmark\s*$")


class PipelineBenchListError(RuntimeError):
    """The compiled pipeline benchmark list disagrees with its lane contract."""


def _pipeline_lane_groups(
    corpus: Corpus,
    *,
    enabled_features: frozenset[str],
) -> tuple[dict[str, str], dict[str, str]]:
    if corpus.schema_version != 2:
        raise PipelineBenchListError("Criterion bench-list verification requires schema_version 2")

    current: dict[str, str] = {}
    historical: dict[str, str] = {}
    for lane in corpus.lanes:
        if lane.transport != "native-criterion":
            continue
        if not set(lane.required_features).issubset(enabled_features):
            continue
        group = lane_selector_group(lane.selector)
        current[group] = lane.id
        for alias in lane.history_aliases:
            historical[lane_selector_group(alias)] = lane.id

    if not current:
        raise PipelineBenchListError(
            "schema-v2 corpus has no native-criterion lanes for the enabled features"
        )
    overlap = sorted(set(current) & set(historical))
    if overlap:
        raise PipelineBenchListError(
            f"current selector groups overlap historical aliases: {overlap}"
        )
    return current, historical


def parse_criterion_bench_list(output: str) -> tuple[str, ...]:
    benches: list[str] = []
    for raw in output.splitlines():
        match = _LIST_LINE.match(strip_ansi(raw).strip())
        if match:
            benches.append(match.group("bench"))
    if not benches:
        raise PipelineBenchListError("Criterion --list returned no benchmark entries")
    duplicates = sorted(bench for bench, count in Counter(benches).items() if count > 1)
    if duplicates:
        raise PipelineBenchListError(
            f"Criterion --list returned duplicate benchmark entries: {duplicates}"
        )
    return tuple(benches)


def _expected_pipeline_benches(
    corpus: Corpus,
    *,
    current_groups: dict[str, str],
) -> set[str]:
    frontmatter_lane = "frontmatter-preprocess-known-type"
    expected: set[str] = set()
    for group, lane_id in current_groups.items():
        expects_frontmatter = lane_id == frontmatter_lane
        for fixture in corpus.fixtures:
            if (fixture.family == "frontmatter") != expects_frontmatter:
                continue
            expected.add(f"{group}/{fixture.name}")
    return expected


def validate_pipeline_bench_list(
    corpus: Corpus,
    output: str,
    *,
    enabled_features: Sequence[str] = ("svg",),
) -> dict[str, object]:
    current, historical = _pipeline_lane_groups(
        corpus,
        enabled_features=frozenset(enabled_features),
    )
    benches = parse_criterion_bench_list(output)
    fixture_names = {fixture.name for fixture in corpus.fixtures}
    if any("/" in name for name in fixture_names):
        raise PipelineBenchListError("corpus fixture names must not contain '/' for Criterion")

    groups: set[str] = set()
    unknown_fixtures: list[str] = []
    for bench in benches:
        if "/" not in bench:
            raise PipelineBenchListError(
                f"Criterion benchmark has no group/fixture boundary: {bench!r}"
            )
        group, fixture = bench.rsplit("/", 1)
        groups.add(group)
        if fixture not in fixture_names:
            unknown_fixtures.append(bench)

    emitted_aliases = sorted(groups & set(historical))
    if emitted_aliases:
        owners = {group: historical[group] for group in emitted_aliases}
        raise PipelineBenchListError(
            f"Criterion emitted historical lane aliases instead of current selectors: {owners}"
        )
    missing = sorted(set(current) - groups)
    unknown = sorted(groups - set(current))
    if missing or unknown:
        raise PipelineBenchListError(
            f"compiled Criterion groups differ from schema-v2 lanes: "
            f"missing={missing}, unknown={unknown}"
        )
    if unknown_fixtures:
        raise PipelineBenchListError(
            f"compiled Criterion list contains fixtures absent from corpus: {sorted(unknown_fixtures)}"
        )

    expected_benches = _expected_pipeline_benches(
        corpus,
        current_groups=current,
    )
    missing_benches = sorted(expected_benches - set(benches))
    unexpected_benches = sorted(set(benches) - expected_benches)
    if missing_benches or unexpected_benches:
        raise PipelineBenchListError(
            "compiled Criterion benchmarks differ from the corpus/lane product: "
            f"missing={missing_benches}, unexpected={unexpected_benches}"
        )

    requires_receipts = any(
        lane.evidence_contract == _NATIVE_CRITERION_PREFLIGHT_CONTRACT
        for lane in corpus.lanes
        if lane.transport == "native-criterion"
        and lane_selector_group(lane.selector) in current
    )
    receipt_count = 0
    if requires_receipts:
        try:
            receipts = parse_preflight_receipts(output)
        except RuntimeError as error:
            raise PipelineBenchListError(str(error)) from error
        missing_receipts = sorted(set(benches) - set(receipts))
        unexpected_receipts = sorted(set(receipts) - set(benches))
        if missing_receipts or unexpected_receipts:
            raise PipelineBenchListError(
                "compiled Criterion preflight receipts differ from the bench list: "
                f"missing={missing_receipts}, unexpected={unexpected_receipts}"
            )
        receipt_count = len(receipts)

    return {
        "bench_count": len(benches),
        "receipt_count": receipt_count,
        "groups": tuple(sorted(groups)),
        "lane_ids": tuple(sorted(current.values())),
    }


def _run(
    command: list[str],
    *,
    root: Path,
    timeout_seconds: int,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.update(
        {
            "CARGO_BUILD_JOBS": "1",
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_BENCH_DEBUG": "0",
        }
    )
    return subprocess.run(
        command,
        cwd=root,
        env=environment,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        check=False,
        timeout=timeout_seconds,
    )


def _require_success(
    result: subprocess.CompletedProcess[str],
    *,
    command: Sequence[str],
) -> None:
    if result.returncode == 0:
        return
    combined = strip_ansi("\n".join((result.stdout, result.stderr))).strip()
    raise PipelineBenchListError(
        f"command failed with exit {result.returncode}: {' '.join(command)}\n{combined[-8_000:]}"
    )


def verify_compiled_pipeline(
    *,
    root: Path,
    corpus_path: Path,
    target_dir: Path,
    features: tuple[str, ...],
    toolchain: str | None,
    timeout_seconds: int,
    package: str = "merman",
    bench: str = "pipeline",
) -> dict[str, object]:
    recipe = RunnerRecipe(
        label=f"{package}-{bench}-bench-list",
        checkout=root,
        package=package,
        bench=bench,
        features=features,
        default_features=False,
        toolchain=toolchain,
        target_dir=target_dir,
        locked=True,
        corpus=corpus_path,
    )
    build_command = cargo_prebuild_command(recipe)
    built = _run(build_command, root=root, timeout_seconds=timeout_seconds)
    _require_success(built, command=build_command)
    executable = parse_bench_executable(built.stdout, recipe=recipe)
    if not executable.is_absolute():
        executable = (root / executable).resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise PipelineBenchListError(f"Cargo reported an unusable bench executable: {executable}")

    list_command = criterion_list_command(executable)
    listed = _run(list_command, root=root, timeout_seconds=timeout_seconds)
    _require_success(listed, command=list_command)
    corpus = load_corpus(corpus_path)
    result = validate_pipeline_bench_list(
        corpus,
        "\n".join((listed.stdout, listed.stderr)),
        enabled_features=features,
    )
    result["executable"] = str(executable)
    return result


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify one compiled Criterion list against schema-v2 lanes."
    )
    parser.add_argument("--repo-root", default=str(ROOT))
    parser.add_argument("--corpus", default="tools/bench/corpus.json")
    parser.add_argument("--target-dir", default="target")
    parser.add_argument("--features", default="svg")
    parser.add_argument("--package", default="merman")
    parser.add_argument("--bench", default="pipeline")
    parser.add_argument("--toolchain")
    parser.add_argument("--timeout-seconds", type=int, default=900)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = _parser().parse_args(list(argv) if argv is not None else None)
    try:
        if args.timeout_seconds <= 0:
            raise PipelineBenchListError("timeout seconds must be positive")
        root = Path(args.repo_root).resolve()
        corpus = Path(args.corpus)
        if not corpus.is_absolute():
            corpus = root / corpus
        target_dir = Path(args.target_dir)
        if not target_dir.is_absolute():
            target_dir = root / target_dir
        features = tuple(
            feature.strip() for feature in args.features.split(",") if feature.strip()
        )
        if not features:
            raise PipelineBenchListError("at least one Cargo feature is required")
        if not args.package or not args.bench:
            raise PipelineBenchListError("package and bench must be non-empty")
        result = verify_compiled_pipeline(
            root=root,
            corpus_path=corpus,
            target_dir=target_dir,
            features=features,
            toolchain=args.toolchain,
            timeout_seconds=args.timeout_seconds,
            package=args.package,
            bench=args.bench,
        )
        print(
            f"Verified {args.package}/{args.bench} Criterion list: "
            f"{result['bench_count']} benches, "
            f"{result['receipt_count']} receipts, "
            f"{len(result['groups'])} lane groups"
        )
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired, PipelineBenchListError) as error:
        print(f"Criterion bench-list contract failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

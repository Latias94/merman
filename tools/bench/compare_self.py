#!/usr/bin/env python3
"""
Compare `merman` Criterion benchmarks between two checkouts.

The diagnostic mode is a CI-friendly advisory. Confirmation mode is the fixed-budget regression
contract: it freezes independent runner recipes, calibrates same-binary noise, and measures fresh
balanced pairs on one host. Cross-repo comparisons remain in `compare_mermaid_renderers.py` because
they answer a different question.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import random
import re
import statistics
import subprocess
import sys
import tomllib
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any, Iterable, Sequence

from compare_mermaid_renderers import (
    best_effort_cpu_model,
    expand_filter_to_exact_benches,
    git_head,
    parse_criterion_times,
    parse_skip_lines,
    pretty_time,
    rustc_verbose,
    split_exact_bench,
    strip_ansi,
)
from corpus_utils import (
    LaneMetadata,
    lane_selector_group,
    load_corpus,
    resolve_lane_group,
    resolve_merman_fixture_path,
    select_corpus_fixtures,
)


DEFAULT_CORPUS = "tools/bench/corpus.json"
DEFAULT_MARKDOWN_OUT = "target/bench/self_comparison.md"
DEFAULT_JSON_OUT = "target/bench/self_comparison.json"
DECISION_GRADE_BOOTSTRAP_RESAMPLES = 10_000
MAX_BOOTSTRAP_RESAMPLES = 100_000


@dataclass(frozen=True)
class RunnerRecipe:
    label: str
    checkout: Path
    package: str
    bench: str
    features: tuple[str, ...]
    default_features: bool
    toolchain: str | None
    target_dir: Path
    locked: bool
    corpus: Path
    target: str | None = None
    logical_operations: int = 1


@dataclass(frozen=True)
class PairCount:
    required_pairs: int
    scheduled_pairs: int
    exceeds_cap: bool


def cargo_prebuild_command(recipe: RunnerRecipe) -> list[str]:
    if not recipe.locked:
        raise ValueError("decision-grade recipes must be locked")
    lockfile = recipe.checkout / "Cargo.lock"
    if not lockfile.is_file():
        raise FileNotFoundError(f"locked recipe is missing Cargo.lock: {lockfile}")
    if recipe.logical_operations <= 0:
        raise ValueError("logical_operations must be positive")

    command = ["cargo"]
    if recipe.toolchain:
        command.append(f"+{recipe.toolchain}")
    command.extend(["bench", "--locked", "-p", recipe.package])
    if not recipe.default_features:
        command.append("--no-default-features")
    if recipe.features:
        command.extend(["--features", ",".join(recipe.features)])
    command.extend(["--bench", recipe.bench])
    if recipe.target:
        command.extend(["--target", recipe.target])
    command.extend(
        [
            "--target-dir",
            str(recipe.target_dir),
            "--no-run",
            "--message-format=json-render-diagnostics",
        ]
    )
    return command


def parse_bench_executable(cargo_stdout: str, *, recipe: RunnerRecipe) -> Path:
    executables: set[Path] = set()
    for raw in cargo_stdout.splitlines():
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
        if target.get("name") != recipe.bench or not isinstance(kinds, list) or "bench" not in kinds:
            continue
        executable = message.get("executable")
        if isinstance(executable, str) and executable:
            executables.add(Path(executable))
    if not executables:
        raise ValueError(f"missing unique executable for bench {recipe.bench!r}")
    if len(executables) != 1:
        rendered = ", ".join(sorted(str(path) for path in executables))
        raise ValueError(f"multiple executables for bench {recipe.bench!r}: {rendered}")
    return next(iter(executables))


def criterion_command(
    *,
    executable: Path,
    exact_bench: str,
    sample_size: int,
    warm_up_seconds: int,
    measurement_seconds: int,
) -> list[str]:
    return [
        str(executable),
        "--bench",
        "--color",
        "never",
        "--noplot",
        "--sample-size",
        str(sample_size),
        "--warm-up-time",
        str(warm_up_seconds),
        "--measurement-time",
        str(measurement_seconds),
        "--discard-baseline",
        "--exact",
        exact_bench,
    ]


def _file_description(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"path": str(path), "bytes": None, "sha256": None}
    data = path.read_bytes()
    return {
        "path": str(path),
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def fixture_byte_comparison(base_path: Path, head_path: Path) -> dict[str, Any]:
    base = _file_description(base_path)
    head = _file_description(head_path)
    if base["sha256"] is None or head["sha256"] is None:
        status = "missing"
    elif base["sha256"] == head["sha256"]:
        status = "identical"
    else:
        status = "different"
    return {"status": status, "base": base, "head": head}


def _percentile(values: Sequence[float], probability: float) -> float:
    if not values:
        raise ValueError("percentile requires at least one value")
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    position = max(0.0, min(1.0, probability)) * (len(ordered) - 1)
    lower = math.floor(position)
    upper = math.ceil(position)
    if lower == upper:
        return ordered[lower]
    fraction = position - lower
    return ordered[lower] * (1.0 - fraction) + ordered[upper] * fraction


def _bootstrap_mean_bounds(
    values: Sequence[float],
    *,
    seed: int,
    resamples: int,
    confidence_level: float,
    interval: str = "one_sided",
) -> dict[str, float]:
    if not values:
        raise ValueError("paired bounds require at least one pair")
    if not 0.0 < confidence_level < 1.0:
        raise ValueError("confidence_level must be between zero and one")
    if resamples <= 0:
        raise ValueError("resamples must be positive")
    if resamples > MAX_BOOTSTRAP_RESAMPLES:
        raise ValueError(
            f"resamples must be at most {MAX_BOOTSTRAP_RESAMPLES}"
        )
    estimate = statistics.fmean(values)
    if len(values) == 1 or all(value == values[0] for value in values):
        return {"estimate": estimate, "lower": estimate, "upper": estimate}
    rng = random.Random(seed)
    n = len(values)
    samples = [statistics.fmean(values[rng.randrange(n)] for _ in range(n)) for _ in range(resamples)]
    alpha = 1.0 - confidence_level
    if interval == "one_sided":
        lower_probability = alpha
        upper_probability = confidence_level
    elif interval == "two_sided":
        lower_probability = alpha / 2.0
        upper_probability = 1.0 - alpha / 2.0
    else:
        raise ValueError(f"unknown interval kind: {interval}")
    return {
        "estimate": estimate,
        "lower": _percentile(samples, lower_probability),
        "upper": _percentile(samples, upper_probability),
    }


def _mirror_bounds(bounds: dict[str, float]) -> dict[str, float]:
    return {
        "estimate": -bounds["estimate"],
        "lower": -bounds["upper"],
        "upper": -bounds["lower"],
    }


def _paired_bounds_with_interval(
    *,
    base_ns: Sequence[float],
    head_ns: Sequence[float],
    confidence_level: float = 0.95,
    bootstrap_seed: int = 0,
    bootstrap_resamples: int = 10_000,
    interval: str,
    family_size: int = 2,
) -> dict[str, Any]:
    if len(base_ns) != len(head_ns) or not base_ns:
        raise ValueError("base/head samples must contain the same non-zero pair count")
    if any(not math.isfinite(value) or value <= 0.0 for value in (*base_ns, *head_ns)):
        raise ValueError("paired samples must be finite and positive")
    if isinstance(family_size, bool) or not isinstance(family_size, int) or family_size <= 0:
        raise ValueError("family_size must be a positive integer")
    component_confidence_level = 1.0 - (1.0 - confidence_level) / family_size
    log_ratios = [math.log(head / base) for base, head in zip(base_ns, head_ns)]
    absolute = [head - base for base, head in zip(base_ns, head_ns)]
    log_bounds = _bootstrap_mean_bounds(
        log_ratios,
        seed=bootstrap_seed,
        resamples=bootstrap_resamples,
        confidence_level=component_confidence_level,
        interval=interval,
    )
    absolute_bounds = _bootstrap_mean_bounds(
        absolute,
        seed=bootstrap_seed ^ 0x5F3759DF,
        resamples=bootstrap_resamples,
        confidence_level=component_confidence_level,
        interval=interval,
    )
    return {
        "log_ratio": log_bounds,
        "absolute_ns": absolute_bounds,
        "improvement_log_ratio": _mirror_bounds(log_bounds),
        "improvement_absolute_ns": _mirror_bounds(absolute_bounds),
        "confidence_contract": {
            "simultaneous_confidence_level": confidence_level,
            "component_confidence_level": component_confidence_level,
            "family_size": family_size,
            "multiplicity_adjustment": "bonferroni",
        },
    }


def paired_bounds(
    *,
    base_ns: Sequence[float],
    head_ns: Sequence[float],
    confidence_level: float = 0.95,
    bootstrap_seed: int = 0,
    bootstrap_resamples: int = 10_000,
    family_size: int = 2,
) -> dict[str, Any]:
    """Return deterministic paired one-sided bounds in the canonical head/base direction."""
    return _paired_bounds_with_interval(
        base_ns=base_ns,
        head_ns=head_ns,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed,
        bootstrap_resamples=bootstrap_resamples,
        interval="one_sided",
        family_size=family_size,
    )


def classify_confirmation(
    bounds: dict[str, dict[str, float]],
    *,
    relative_threshold: float,
    absolute_threshold_ns: float,
    direction: str,
    evidence_mode: str,
    pair_count: int,
    required_pairs: int,
) -> str:
    if evidence_mode == "diagnostic":
        if not 1 <= pair_count <= 4:
            raise ValueError("diagnostic evidence requires one to four pairs")
        return "diagnostic_advisory"
    if evidence_mode != "confirmation":
        raise ValueError(f"unknown evidence mode: {evidence_mode}")
    if pair_count < 8 or pair_count < required_pairs:
        return "inconclusive"

    relative = bounds["log_ratio"]
    absolute = bounds["absolute_ns"]
    if direction == "regression":
        if relative["lower"] > relative_threshold and absolute["lower"] > absolute_threshold_ns:
            return "confirmed_regression"
        if relative["upper"] <= relative_threshold or absolute["upper"] <= absolute_threshold_ns:
            return "confirmed_non_regression"
        return "inconclusive"
    if direction == "improvement":
        if relative["upper"] < -relative_threshold and absolute["upper"] < -absolute_threshold_ns:
            return "confirmed_improvement"
        if relative["lower"] >= -relative_threshold or absolute["lower"] >= -absolute_threshold_ns:
            return "confirmed_non_improvement"
        return "inconclusive"
    raise ValueError(f"unknown confirmation direction: {direction}")


def required_pair_count(
    *, sigma: float, minimum_detectable_effect: float, max_pairs: int
) -> PairCount:
    if not math.isfinite(sigma) or sigma < 0.0:
        raise ValueError("sigma must be finite and non-negative")
    if not math.isfinite(minimum_detectable_effect) or minimum_detectable_effect <= 0.0:
        raise ValueError("minimum_detectable_effect must be finite and positive")
    if max_pairs < 8:
        raise ValueError("max_pairs must be at least eight")
    raw = max(8, math.ceil(((1.645 + 0.842) * sigma / minimum_detectable_effect) ** 2))
    required = raw if raw % 2 == 0 else raw + 1
    return PairCount(
        required_pairs=required,
        scheduled_pairs=min(required, max_pairs),
        exceeds_cap=required > max_pairs,
    )


def suite_exit_code(outcomes: Iterable[str]) -> int:
    values = set(outcomes)
    if "contract_failure" in values:
        return 2
    if "confirmed_regression" in values:
        return 1
    if "inconclusive" in values:
        return 3
    return 0


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


def write_json_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def fmt_time(value: float | None) -> str:
    if value is None:
        return "-"
    return pretty_time(value)


_BENCH_LIST_LINE = re.compile(r"^(?P<bench>[A-Za-z0-9_.:/-]+):\s*benchmark\s*$")


class ContractViolation(RuntimeError):
    pass


@dataclass
class PreparedRunner:
    recipe: RunnerRecipe
    executable: Path
    executable_sha256: str
    benches: set[str]
    skipped: dict[str, list[str]]
    provenance: dict[str, Any]
    env: dict[str, str]


def criterion_list_command(executable: Path) -> list[str]:
    return [
        str(executable),
        "--bench",
        "--color",
        "never",
        "--list",
        "--format",
        "terse",
    ]


def _command_text(command: Sequence[str]) -> str:
    return " ".join(command)


def _output_tail(text: str, *, limit: int = 8_000) -> str:
    value = strip_ansi(text).strip()
    return value if len(value) <= limit else value[-limit:] + "\n... <tail truncated>"


def _run_process(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout_seconds: int,
) -> subprocess.CompletedProcess[str]:
    process_env = os.environ.copy()
    if env:
        process_env.update(env)
    return subprocess.run(
        command,
        cwd=str(cwd),
        env=process_env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
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
    cwd: Path,
) -> None:
    if result.returncode == 0:
        return
    output = _output_tail("\n".join((result.stdout, result.stderr)))
    raise ContractViolation(
        f"command failed (exit {result.returncode}) in {cwd}: "
        f"{_command_text(command)}\n{output}"
    )


def _path_sha256(path: Path) -> str:
    if not path.is_file():
        raise ContractViolation(f"required file is missing: {path}")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _describe_required_file(
    path: Path,
    *,
    sha256: str | None = None,
) -> dict[str, Any]:
    return {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256 if sha256 is not None else _path_sha256(path),
    }


def _find_package_manifest(checkout: Path, package: str) -> Path:
    candidates = [checkout / "Cargo.toml"]
    for parent in (checkout / "crates", checkout / "platforms"):
        if parent.is_dir():
            candidates.extend(sorted(parent.glob("*/Cargo.toml")))
    for path in candidates:
        if not path.is_file():
            continue
        try:
            data = tomllib.loads(path.read_text(encoding="utf-8"))
        except (OSError, tomllib.TOMLDecodeError):
            continue
        package_table = data.get("package")
        if isinstance(package_table, dict) and package_table.get("name") == package:
            return path
    raise ContractViolation(f"could not resolve Cargo manifest for package {package!r}")


def _git_provenance(
    checkout: Path,
    *,
    allow_dirty: bool,
    timeout_seconds: int,
) -> dict[str, Any]:
    revision = git_head(checkout)
    if not revision:
        raise ContractViolation(f"could not resolve git revision in {checkout}")
    command = ["git", "status", "--porcelain=v1", "--untracked-files=normal"]
    result = _run_process(command, cwd=checkout, timeout_seconds=timeout_seconds)
    _require_success(result, command=command, cwd=checkout)
    dirty_entries = [line for line in result.stdout.splitlines() if line.strip()]
    if dirty_entries and not allow_dirty:
        raise ContractViolation(
            f"checkout is dirty ({len(dirty_entries)} entries); "
            "use --allow-dirty only for explicitly diagnostic evidence"
        )
    return {
        "revision": revision,
        "dirty": bool(dirty_entries),
        "dirty_disposition": "explicitly_allowed" if dirty_entries else "clean",
        "dirty_entries": dirty_entries[:100],
        "dirty_entries_truncated": len(dirty_entries) > 100,
    }


def _toolchain_version(recipe: RunnerRecipe, *, timeout_seconds: int) -> str:
    command = ["rustc"]
    if recipe.toolchain:
        command.append(f"+{recipe.toolchain}")
    command.append("-Vv")
    result = _run_process(command, cwd=recipe.checkout, timeout_seconds=timeout_seconds)
    _require_success(result, command=command, cwd=recipe.checkout)
    return result.stdout.strip()


def _cargo_version(recipe: RunnerRecipe, *, timeout_seconds: int) -> str:
    command = ["cargo"]
    if recipe.toolchain:
        command.append(f"+{recipe.toolchain}")
    command.append("-Vv")
    result = _run_process(command, cwd=recipe.checkout, timeout_seconds=timeout_seconds)
    _require_success(result, command=command, cwd=recipe.checkout)
    return result.stdout.strip()


def _recipe_report(recipe: RunnerRecipe) -> dict[str, Any]:
    return {
        "label": recipe.label,
        "checkout": str(recipe.checkout),
        "package": recipe.package,
        "bench": recipe.bench,
        "features": list(recipe.features),
        "default_features": recipe.default_features,
        "toolchain": recipe.toolchain,
        "target": recipe.target,
        "target_dir": str(recipe.target_dir),
        "cargo_profile": "bench",
        "locked": recipe.locked,
        "corpus": str(recipe.corpus),
        "logical_operations": recipe.logical_operations,
        "logical_operations_scope": "selected_group_uniform_override",
    }


def _prepare_runner(
    recipe: RunnerRecipe,
    *,
    allow_dirty: bool,
    timeout_seconds: int,
) -> tuple[PreparedRunner | None, dict[str, Any], list[str]]:
    provenance: dict[str, Any] = {"recipe": _recipe_report(recipe)}
    errors: list[str] = []
    try:
        if not recipe.checkout.is_dir():
            raise ContractViolation(f"checkout is not a directory: {recipe.checkout}")
        provenance["git"] = _git_provenance(
            recipe.checkout,
            allow_dirty=allow_dirty,
            timeout_seconds=timeout_seconds,
        )
        recipe.target_dir.mkdir(parents=True, exist_ok=True)
        manifest = _find_package_manifest(recipe.checkout, recipe.package)
        workspace_manifest = recipe.checkout / "Cargo.toml"
        lockfile = recipe.checkout / "Cargo.lock"
        corpus_path = recipe.corpus if recipe.corpus.is_absolute() else recipe.checkout / recipe.corpus
        provenance["manifest"] = _describe_required_file(manifest)
        provenance["workspace_manifest"] = _describe_required_file(workspace_manifest)
        provenance["lockfile"] = _describe_required_file(lockfile)
        provenance["corpus"] = _describe_required_file(corpus_path)
        bench_source = manifest.parent / "benches" / f"{recipe.bench}.rs"
        provenance["bench_source"] = _describe_required_file(bench_source)
        provenance["toolchain"] = {
            "requested": recipe.toolchain,
            "rustc_verbose": _toolchain_version(recipe, timeout_seconds=timeout_seconds),
            "cargo_verbose": _cargo_version(recipe, timeout_seconds=timeout_seconds),
        }
        provenance["build_environment"] = {
            key: os.environ.get(key)
            for key in (
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "CARGO_PROFILE_BENCH_LTO",
                "CARGO_PROFILE_BENCH_CODEGEN_UNITS",
                "CARGO_PROFILE_BENCH_OPT_LEVEL",
            )
        }

        env = {
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_BENCH_DEBUG": "0",
        }
        command = cargo_prebuild_command(recipe)
        provenance["prebuild_command"] = command
        print(f"[prepare] {recipe.label}: {_command_text(command)}", flush=True)
        result = _run_process(
            command,
            cwd=recipe.checkout,
            env=env,
            timeout_seconds=timeout_seconds,
        )
        provenance["prebuild_stderr_tail"] = _output_tail(result.stderr)
        _require_success(result, command=command, cwd=recipe.checkout)
        executable = parse_bench_executable(result.stdout, recipe=recipe)
        if not executable.is_absolute():
            executable = (recipe.checkout / executable).resolve()
        if not executable.is_file():
            raise ContractViolation(f"Cargo reported a missing bench executable: {executable}")
        if not os.access(executable, os.X_OK):
            raise ContractViolation(f"bench executable is not executable: {executable}")
        executable_sha256 = _path_sha256(executable)
        provenance["executable"] = {
            **_describe_required_file(executable, sha256=executable_sha256),
            "executable": True,
        }

        list_command = criterion_list_command(executable)
        provenance["discovery_command"] = list_command
        print(f"[discover] {recipe.label}: {_command_text(list_command)}", flush=True)
        listed = _run_process(
            list_command,
            cwd=recipe.checkout,
            env=env,
            timeout_seconds=timeout_seconds,
        )
        _require_success(listed, command=list_command, cwd=recipe.checkout)
        combined = "\n".join((listed.stdout, listed.stderr))
        benches: set[str] = set()
        for raw in combined.splitlines():
            match = _BENCH_LIST_LINE.match(strip_ansi(raw).strip())
            if match:
                benches.add(match.group("bench"))
        if not benches:
            raise ContractViolation(
                f"Criterion discovery returned no benchmarks for {recipe.label}"
            )
        skipped = parse_skip_lines(combined)
        provenance["discovery"] = {
            "bench_count": len(benches),
            "benches": sorted(benches),
            "skipped": skipped,
            "output_sha256": hashlib.sha256(combined.encode("utf-8")).hexdigest(),
        }
        if _path_sha256(lockfile) != provenance["lockfile"]["sha256"]:
            raise ContractViolation(f"Cargo.lock changed while preparing {recipe.label}")
        if _path_sha256(executable) != executable_sha256:
            raise ContractViolation(
                f"bench executable changed while discovering {recipe.label}"
            )
        return (
            PreparedRunner(
                recipe=recipe,
                executable=executable,
                executable_sha256=executable_sha256,
                benches=benches,
                skipped=skipped,
                provenance=provenance,
                env=env,
            ),
            provenance,
            errors,
        )
    except Exception as error:
        errors.append(str(error))
        provenance["preparation_error"] = str(error)
        return None, provenance, errors


def _stable_seed(seed: int, *parts: object) -> int:
    material = "\x1f".join((str(seed), *(str(part) for part in parts)))
    return int.from_bytes(hashlib.sha256(material.encode("utf-8")).digest()[:8], "big")


def _availability(runner: PreparedRunner, exact: str | None) -> str:
    if exact is None:
        return "not_selected"
    if exact in runner.benches:
        return "available"
    group, name = split_exact_bench(exact)
    if name in runner.skipped.get(group, []):
        return "skipped"
    return "missing"


def _fixture_metadata(fixture: Any | None) -> dict[str, Any] | None:
    if fixture is None:
        return None
    return {
        "name": fixture.name,
        "family": fixture.family,
        "size": fixture.size,
        "category": fixture.category,
        "source": fixture.source,
        "suites": list(fixture.suites),
        "features": list(fixture.features),
        "quality": list(fixture.quality),
    }


def _lane_metadata(lane: LaneMetadata | None) -> dict[str, Any] | None:
    return asdict(lane) if lane is not None else None


def _optional_lane_for_group(corpus: Any, group: str | None) -> LaneMetadata | None:
    if group is None or not corpus.lanes:
        return None
    try:
        return resolve_lane_group(corpus, group)
    except ValueError as error:
        if corpus.schema_version >= 2:
            raise ContractViolation(
                f"schema-v2 corpus has no registered lane for benchmark group {group!r}"
            ) from error
        return None


def _lane_matches_group(lane: LaneMetadata, group: str) -> bool:
    return any(
        lane_selector_group(selector) == group
        for selector in (lane.selector, *lane.history_aliases)
    )


def _effective_operation_lanes(
    *,
    base_lane: LaneMetadata | None,
    head_lane: LaneMetadata | None,
    base_group: str | None,
    head_group: str | None,
) -> tuple[LaneMetadata | None, LaneMetadata | None]:
    effective_base = base_lane
    effective_head = head_lane
    if (
        effective_base is None
        and head_lane is not None
        and base_group is not None
        and _lane_matches_group(head_lane, base_group)
    ):
        effective_base = head_lane
    if (
        effective_head is None
        and base_lane is not None
        and head_group is not None
        and _lane_matches_group(base_lane, head_group)
    ):
        effective_head = base_lane
    return effective_base, effective_head


def _recipe_with_lane_divisor(
    recipe: RunnerRecipe,
    *,
    lane: LaneMetadata | None,
    explicit_divisor: int | None,
) -> RunnerRecipe:
    if lane is None:
        divisor = explicit_divisor if explicit_divisor is not None else 1
    else:
        divisor = lane.logical_operations_per_estimate
        if explicit_divisor is not None and explicit_divisor != divisor:
            raise ContractViolation(
                f"{recipe.label} logical-operation divisor {explicit_divisor} conflicts "
                f"with lane {lane.id!r} divisor {divisor}"
            )
    return replace(recipe, logical_operations=divisor)


def _load_fixture_contracts(
    *,
    base_recipe: RunnerRecipe,
    head_recipe: RunnerRecipe,
    suite: str,
    common_group: str | None,
    base_group_override: str | None,
    head_group_override: str | None,
    filter_expr: str | None,
) -> tuple[
    list[dict[str, Any]],
    dict[str, Any],
    tuple[LaneMetadata | None, LaneMetadata | None],
]:
    base_corpus_path = (
        base_recipe.corpus
        if base_recipe.corpus.is_absolute()
        else base_recipe.checkout / base_recipe.corpus
    )
    head_corpus_path = (
        head_recipe.corpus
        if head_recipe.corpus.is_absolute()
        else head_recipe.checkout / head_recipe.corpus
    )
    base_corpus = load_corpus(base_corpus_path)
    head_corpus = load_corpus(head_corpus_path)
    base_metadata = {fixture.name: fixture for fixture in base_corpus.fixtures}
    head_metadata = {fixture.name: fixture for fixture in head_corpus.fixtures}

    if filter_expr:
        exact_filters = expand_filter_to_exact_benches(filter_expr)
        requested: list[tuple[str, str, str]] = []
        for exact in exact_filters:
            _, name = split_exact_bench(exact)
            requested.append((name, exact, exact))
        base_selected = {name for name, _, _ in requested}
        head_selected = set(base_selected)
        filter_groups = {split_exact_bench(exact)[0] for exact in exact_filters}
        base_group = next(iter(filter_groups)) if len(filter_groups) == 1 else None
        head_group = base_group
        selection = {
            "kind": "filter",
            "filter": filter_expr,
            "suite": None,
            "group": None,
            "groups": {"base": base_group, "head": head_group},
        }
    else:
        base_fixtures = select_corpus_fixtures(base_corpus, suite)
        head_fixtures = select_corpus_fixtures(head_corpus, suite)
        base_selected = {fixture.name for fixture in base_fixtures}
        head_selected = {fixture.name for fixture in head_fixtures}
        names = list(dict.fromkeys([fixture.name for fixture in base_fixtures + head_fixtures]))
        base_group = base_group_override or common_group or base_corpus.default_group
        head_group = head_group_override or common_group or head_corpus.default_group
        requested = [
            (
                name,
                f"{base_group}/{name}" if name in base_selected else "",
                f"{head_group}/{name}" if name in head_selected else "",
            )
            for name in names
        ]
        selection = {
            "kind": "suite",
            "filter": None,
            "suite": suite,
            "group": base_group if base_group == head_group else None,
            "groups": {"base": base_group, "head": head_group},
        }

    contracts: list[dict[str, Any]] = []
    for name, base_exact_raw, head_exact_raw in requested:
        base_fixture = base_metadata.get(name)
        head_fixture = head_metadata.get(name)
        base_path = resolve_merman_fixture_path(base_recipe.checkout, name, base_fixture)
        head_path = resolve_merman_fixture_path(head_recipe.checkout, name, head_fixture)
        byte_contract = fixture_byte_comparison(base_path, head_path)
        contracts.append(
            {
                "name": name,
                "family": (
                    head_fixture.family
                    if head_fixture is not None
                    else base_fixture.family if base_fixture is not None else "unknown"
                ),
                "base_benchmark": base_exact_raw or None,
                "head_benchmark": head_exact_raw or None,
                "selected": {
                    "base": name in base_selected,
                    "head": name in head_selected,
                },
                "metadata": {
                    "base": _fixture_metadata(base_fixture),
                    "head": _fixture_metadata(head_fixture),
                },
                "bytes": byte_contract,
            }
        )
    selection["corpora"] = {
        "base": _describe_required_file(base_corpus_path),
        "head": _describe_required_file(head_corpus_path),
    }
    base_lane = _optional_lane_for_group(base_corpus, base_group)
    head_lane = _optional_lane_for_group(head_corpus, head_group)
    effective_lanes = _effective_operation_lanes(
        base_lane=base_lane,
        head_lane=head_lane,
        base_group=base_group,
        head_group=head_group,
    )
    selection["lane_contracts"] = {
        "declared": {
            "base": _lane_metadata(base_lane),
            "head": _lane_metadata(head_lane),
        },
        "effective": {
            "base": _lane_metadata(effective_lanes[0]),
            "head": _lane_metadata(effective_lanes[1]),
        },
    }
    return contracts, selection, effective_lanes


def _complete_fixture_contracts(
    contracts: list[dict[str, Any]],
    *,
    base: PreparedRunner,
    head: PreparedRunner,
) -> list[dict[str, Any]]:
    for contract in contracts:
        base_status = _availability(base, contract["base_benchmark"])
        head_status = _availability(head, contract["head_benchmark"])
        contract["capability"] = {"base": base_status, "head": head_status}
        reasons: list[str] = []
        if not contract["selected"]["base"] or not contract["selected"]["head"]:
            reasons.append("fixture is not selected by both corpus recipes")
        if contract["bytes"]["status"] != "identical":
            reasons.append(f"fixture bytes are {contract['bytes']['status']}")
        if base_status != "available" or head_status != "available":
            reasons.append(f"benchmark capability base={base_status}, head={head_status}")
        contract["coverage_status"] = "comparable" if not reasons else "coverage_only"
        contract["coverage_reasons"] = reasons
    return contracts


def _measure_once(
    runner: PreparedRunner,
    *,
    exact_bench: str,
    sample_size: int,
    warm_up_seconds: int,
    measurement_seconds: int,
    timeout_seconds: int,
    sequence_index: int,
) -> dict[str, Any]:
    command = criterion_command(
        executable=runner.executable,
        exact_bench=exact_bench,
        sample_size=sample_size,
        warm_up_seconds=warm_up_seconds,
        measurement_seconds=measurement_seconds,
    )
    print(
        f"[measure {sequence_index}] {runner.recipe.label}: {exact_bench}",
        flush=True,
    )
    result = _run_process(
        command,
        cwd=runner.recipe.checkout,
        env=runner.env,
        timeout_seconds=timeout_seconds,
    )
    _require_success(result, command=command, cwd=runner.recipe.checkout)
    output = "\n".join((result.stdout, result.stderr))
    prefix, name = split_exact_bench(exact_bench)
    estimate = parse_criterion_times(output, prefix=prefix).get(name)
    if estimate is None:
        skip_map = parse_skip_lines(output)
        reason = skip_map.get(prefix, [])
        detail = f"; skip={reason}" if reason else ""
        raise ContractViolation(
            f"Criterion output did not contain a point estimate for {exact_bench}{detail}: "
            f"{_output_tail(output)}"
        )
    raw_ns = estimate.to_nanos()
    normalized_ns = raw_ns / runner.recipe.logical_operations
    if not math.isfinite(normalized_ns) or normalized_ns <= 0.0:
        raise ContractViolation(
            f"invalid normalized estimate for {exact_bench}: {normalized_ns} ns"
        )
    return {
        "runner": runner.recipe.label,
        "sequence_index": sequence_index,
        "benchmark": exact_bench,
        "raw_estimate_ns": raw_ns,
        "logical_operations": runner.recipe.logical_operations,
        "normalized_ns": normalized_ns,
        "command": command,
        "output_sha256": hashlib.sha256(output.encode("utf-8")).hexdigest(),
    }


def _run_aa_schedule(
    runner: PreparedRunner,
    *,
    contracts: list[dict[str, Any]],
    pair_count: int,
    sample_size: int,
    warm_up_seconds: int,
    measurement_seconds: int,
    timeout_seconds: int,
) -> dict[str, Any]:
    rows: dict[str, list[dict[str, Any]]] = {item["name"]: [] for item in contracts}
    rounds: list[dict[str, Any]] = []
    errors: dict[str, str] = {}
    failed: set[str] = set()
    sequence_index = 0
    side_key = runner.recipe.label
    for pair_index in range(pair_count):
        roles = ("a", "b") if pair_index % 2 == 0 else ("b", "a")
        round_record: dict[str, Any] = {
            "pair_index": pair_index,
            "order": "AB" if roles[0] == "a" else "BA",
            "benchmarks": {},
        }
        for contract in contracts:
            name = contract["name"]
            if name in failed:
                continue
            exact = contract[f"{side_key}_benchmark"]
            record: dict[str, Any] = {
                "pair_index": pair_index,
                "order": round_record["order"],
            }
            try:
                for role in roles:
                    sequence_index += 1
                    record[role] = _measure_once(
                        runner,
                        exact_bench=exact,
                        sample_size=sample_size,
                        warm_up_seconds=warm_up_seconds,
                        measurement_seconds=measurement_seconds,
                        timeout_seconds=timeout_seconds,
                        sequence_index=sequence_index,
                    )
                record["first"] = record[roles[0]]
                record["second"] = record[roles[1]]
                rows[name].append(record)
            except Exception as error:
                record["error"] = str(error)
                errors[name] = str(error)
                failed.add(name)
            round_record["benchmarks"][name] = record
        rounds.append(round_record)
    return {"pair_count": pair_count, "rounds": rounds, "rows": rows, "errors": errors}


def _run_ab_schedule(
    *,
    base: PreparedRunner,
    head: PreparedRunner,
    contracts: list[dict[str, Any]],
    pair_count: int,
    start_side: str,
    sample_size: int,
    warm_up_seconds: int,
    measurement_seconds: int,
    timeout_seconds: int,
) -> dict[str, Any]:
    rows: dict[str, list[dict[str, Any]]] = {item["name"]: [] for item in contracts}
    rounds: list[dict[str, Any]] = []
    errors: dict[str, str] = {}
    failed: set[str] = set()
    sequence_index = 0
    runners = {"base": base, "head": head}
    for pair_index in range(pair_count):
        first = start_side if pair_index % 2 == 0 else ("head" if start_side == "base" else "base")
        second = "head" if first == "base" else "base"
        order = (first, second)
        round_record: dict[str, Any] = {
            "pair_index": pair_index,
            "order": "AB" if order == ("base", "head") else "BA",
            "benchmarks": {},
        }
        for contract in contracts:
            name = contract["name"]
            if name in failed:
                continue
            record: dict[str, Any] = {
                "pair_index": pair_index,
                "order": round_record["order"],
            }
            try:
                for side in order:
                    sequence_index += 1
                    record[side] = _measure_once(
                        runners[side],
                        exact_bench=contract[f"{side}_benchmark"],
                        sample_size=sample_size,
                        warm_up_seconds=warm_up_seconds,
                        measurement_seconds=measurement_seconds,
                        timeout_seconds=timeout_seconds,
                        sequence_index=sequence_index,
                    )
                rows[name].append(record)
            except Exception as error:
                record["error"] = str(error)
                errors[name] = str(error)
                failed.add(name)
            round_record["benchmarks"][name] = record
        rounds.append(round_record)
    return {"pair_count": pair_count, "rounds": rounds, "rows": rows, "errors": errors}


def _bounds_with_relative_percent(bounds: dict[str, Any]) -> dict[str, Any]:
    relative = {
        key: math.expm1(value) * 100.0
        for key, value in bounds["log_ratio"].items()
    }
    improvement_relative = {
        key: math.expm1(value) * 100.0
        for key, value in bounds["improvement_log_ratio"].items()
    }
    return {
        **bounds,
        "relative_percent": relative,
        "improvement_relative_percent": improvement_relative,
    }


def _within_equivalence_margin(bounds: dict[str, float], margin: float) -> bool:
    return bounds["lower"] >= -margin and bounds["upper"] <= margin


def _summarize_calibration(
    pairs: list[dict[str, Any]],
    *,
    expected_pairs: int,
    relative_mde: float,
    absolute_mde_ns: float,
    max_pairs: int,
    confidence_level: float,
    bootstrap_seed: int,
    bootstrap_resamples: int,
    confidence_family_size: int = 2,
) -> dict[str, Any]:
    if len(pairs) != expected_pairs:
        return {
            "status": "contract_failure",
            "stable": False,
            "complete_pairs": len(pairs),
            "expected_pairs": expected_pairs,
            "reason": "A/A schedule did not produce every preregistered pair",
        }
    role_a = [pair["a"]["normalized_ns"] for pair in pairs]
    role_b = [pair["b"]["normalized_ns"] for pair in pairs]
    first = [pair["first"]["normalized_ns"] for pair in pairs]
    second = [pair["second"]["normalized_ns"] for pair in pairs]
    identity = _paired_bounds_with_interval(
        base_ns=role_a,
        head_ns=role_b,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed,
        bootstrap_resamples=bootstrap_resamples,
        interval="two_sided",
        family_size=confidence_family_size,
    )
    order_effect = _paired_bounds_with_interval(
        base_ns=first,
        head_ns=second,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed ^ 0xA5A5A5A5,
        bootstrap_resamples=bootstrap_resamples,
        interval="two_sided",
        family_size=confidence_family_size,
    )
    log_values = [math.log(value_b / value_a) for value_a, value_b in zip(role_a, role_b)]
    absolute_values = [value_b - value_a for value_a, value_b in zip(role_a, role_b)]
    sigma_log = statistics.stdev(log_values)
    sigma_absolute = statistics.stdev(absolute_values)
    relative_count = required_pair_count(
        sigma=sigma_log,
        minimum_detectable_effect=relative_mde,
        max_pairs=max_pairs,
    )
    absolute_count = required_pair_count(
        sigma=sigma_absolute,
        minimum_detectable_effect=absolute_mde_ns,
        max_pairs=max_pairs,
    )
    required_pairs = max(relative_count.required_pairs, absolute_count.required_pairs)
    exceeds_cap = required_pairs > max_pairs
    identity_checks = {
        "identity_log_within_margin": _within_equivalence_margin(
            identity["log_ratio"], relative_mde
        ),
        "identity_absolute_within_margin": _within_equivalence_margin(
            identity["absolute_ns"], absolute_mde_ns
        ),
    }
    order_checks = {
        "order_log_within_margin": _within_equivalence_margin(
            order_effect["log_ratio"], relative_mde
        ),
        "order_absolute_within_margin": _within_equivalence_margin(
            order_effect["absolute_ns"], absolute_mde_ns
        ),
    }
    checks = {
        **identity_checks,
        **order_checks,
        "identity_within_equivalence_margin": all(identity_checks.values()),
        "order_effect_within_equivalence_margin": all(order_checks.values()),
        "required_pairs_fit_cap": not exceeds_cap,
    }
    stable = (
        checks["identity_within_equivalence_margin"]
        and checks["order_effect_within_equivalence_margin"]
        and checks["required_pairs_fit_cap"]
    )
    reasons = [
        name
        for name, passed in {**identity_checks, **order_checks}.items()
        if not passed
    ]
    if exceeds_cap:
        reasons.append("required_pairs_fit_cap")
    return {
        "status": "stable" if stable else "inconclusive",
        "stable": stable,
        "complete_pairs": len(pairs),
        "expected_pairs": expected_pairs,
        "identity_bounds": _bounds_with_relative_percent(identity),
        "order_effect_bounds": _bounds_with_relative_percent(order_effect),
        "sigma": {"log_ratio": sigma_log, "absolute_ns": sigma_absolute},
        "required_pairs": {
            "relative": asdict(relative_count),
            "absolute": asdict(absolute_count),
            "maximum": required_pairs,
            "exceeds_cap": exceeds_cap,
        },
        "checks": checks,
        "equivalence_margins": {
            "log_ratio": relative_mde,
            "absolute_ns": absolute_mde_ns,
        },
        "reasons": reasons,
        "bootstrap": {
            "seed": bootstrap_seed,
            "resamples": bootstrap_resamples,
            "confidence_level": confidence_level,
            "confidence_family_size": confidence_family_size,
            "multiplicity_adjustment": "bonferroni",
            "interval": "two_sided",
        },
    }


def _verification_errors(
    runner: PreparedRunner, *, timeout_seconds: int
) -> list[str]:
    errors: list[str] = []
    verification: dict[str, Any] = {"files": {}}
    try:
        current = _path_sha256(runner.executable)
        verification["executable_sha256"] = current
        if current != runner.executable_sha256:
            errors.append(f"{runner.recipe.label} executable digest changed during sampling")
    except Exception as error:
        errors.append(str(error))
    for key in ("manifest", "workspace_manifest", "lockfile", "corpus", "bench_source"):
        try:
            frozen = runner.provenance[key]
            current = _path_sha256(Path(frozen["path"]))
            verification["files"][key] = current
            if current != frozen["sha256"]:
                errors.append(
                    f"{runner.recipe.label} {key} digest changed during sampling"
                )
        except Exception as error:
            errors.append(str(error))
    try:
        current_git = _git_provenance(
            runner.recipe.checkout,
            allow_dirty=True,
            timeout_seconds=timeout_seconds,
        )
        frozen_git = runner.provenance["git"]
        verification["git"] = current_git
        if current_git["revision"] != frozen_git["revision"]:
            errors.append(f"{runner.recipe.label} git revision changed during sampling")
        if current_git["dirty_entries"] != frozen_git["dirty_entries"]:
            errors.append(f"{runner.recipe.label} dirty tree changed during sampling")
    except Exception as error:
        errors.append(str(error))
    verification["status"] = "verified" if not errors else "failed"
    runner.provenance["post_sampling_verification"] = verification
    return errors


def _fixture_verification_errors(contracts: Sequence[dict[str, Any]]) -> list[str]:
    errors: list[str] = []
    for contract in contracts:
        verification: dict[str, Any] = {}
        for side in ("base", "head"):
            frozen = contract["bytes"][side]
            path = Path(frozen["path"])
            current = _file_description(path)
            verification[side] = current
            if current["sha256"] != frozen["sha256"] or current["bytes"] != frozen["bytes"]:
                errors.append(
                    f"{side} fixture {contract['name']} changed during sampling: {path}"
                )
        verification["status"] = (
            "verified"
            if all(
                verification[side]["sha256"] == contract["bytes"][side]["sha256"]
                and verification[side]["bytes"] == contract["bytes"][side]["bytes"]
                for side in ("base", "head")
            )
            else "failed"
        )
        contract["post_sampling_verification"] = verification
    return errors


def _comparison_row(
    contract: dict[str, Any],
    *,
    pairs: list[dict[str, Any]],
    evidence_mode: str,
    required_pairs: int,
    relative_threshold: float,
    absolute_threshold_ns: float,
    confidence_level: float,
    bootstrap_seed: int,
    bootstrap_resamples: int,
    confidence_family_size: int = 2,
    forced_outcome: str | None = None,
    reason: str | None = None,
) -> dict[str, Any]:
    row: dict[str, Any] = {
        "benchmark": contract["head_benchmark"] or contract["base_benchmark"] or contract["name"],
        "fixture": contract["name"],
        "family": contract["family"],
        "base_benchmark": contract["base_benchmark"],
        "head_benchmark": contract["head_benchmark"],
        "coverage_status": contract.get("coverage_status"),
        "pair_count": len(pairs),
        "required_pairs": required_pairs,
        "evidence_mode": evidence_mode,
        "bootstrap": {
            "seed": bootstrap_seed,
            "resamples": bootstrap_resamples,
            "confidence_level": confidence_level,
            "interval": "one_sided",
            "confidence_family_size": confidence_family_size,
            "multiplicity_adjustment": "bonferroni",
        },
        "raw_pairs": pairs,
        "reason": reason,
        "base_ns": None,
        "head_ns": None,
        "bounds": None,
        "improvement_outcome": None,
    }
    if forced_outcome is not None:
        row["outcome"] = forced_outcome
        return row
    base_values = [pair["base"]["normalized_ns"] for pair in pairs]
    head_values = [pair["head"]["normalized_ns"] for pair in pairs]
    if not base_values or len(base_values) != len(head_values):
        row["outcome"] = "contract_failure"
        row["reason"] = "comparison schedule did not produce complete pairs"
        return row
    bounds = paired_bounds(
        base_ns=base_values,
        head_ns=head_values,
        confidence_level=confidence_level,
        bootstrap_seed=bootstrap_seed,
        bootstrap_resamples=bootstrap_resamples,
        family_size=confidence_family_size,
    )
    row["base_ns"] = statistics.fmean(base_values)
    row["head_ns"] = statistics.fmean(head_values)
    row["bounds"] = _bounds_with_relative_percent(bounds)
    row["outcome"] = classify_confirmation(
        bounds,
        relative_threshold=relative_threshold,
        absolute_threshold_ns=absolute_threshold_ns,
        direction="regression",
        evidence_mode=evidence_mode,
        pair_count=len(pairs),
        required_pairs=required_pairs,
    )
    row["improvement_outcome"] = classify_confirmation(
        bounds,
        relative_threshold=relative_threshold,
        absolute_threshold_ns=absolute_threshold_ns,
        direction="improvement",
        evidence_mode=evidence_mode,
        pair_count=len(pairs),
        required_pairs=required_pairs,
    )
    return row


def _append_contract_error(report: dict[str, Any], stage: str, message: str) -> None:
    report.setdefault("contract_errors", []).append({"stage": stage, "message": message})


def _finalize_summary(report: dict[str, Any]) -> int:
    rows = report.get("rows", [])
    outcomes = [row.get("outcome", "contract_failure") for row in rows]
    if report.get("contract_errors"):
        outcomes.append("contract_failure")
    exit_code = suite_exit_code(outcomes)
    if "contract_failure" in outcomes:
        outcome = "contract_failure"
    elif "confirmed_regression" in outcomes:
        outcome = "confirmed_regression"
    elif "inconclusive" in outcomes:
        outcome = "inconclusive"
    elif report["method"]["discovery_only"] or report["method"]["evidence_mode"] == "diagnostic":
        outcome = "diagnostic_advisory"
    else:
        outcome = "confirmed_non_regression"
    comparable = sum(
        1 for fixture in report.get("fixtures", []) if fixture.get("coverage_status") == "comparable"
    )
    report["summary"] = {
        "outcome": outcome,
        "exit_code": exit_code,
        "comparable": comparable,
        "contract_failures": len(report.get("contract_errors", []))
        + sum(1 for row in rows if row.get("outcome") == "contract_failure"),
        "confirmed_regressions": sum(
            1 for row in rows if row.get("outcome") == "confirmed_regression"
        ),
        "inconclusive": sum(1 for row in rows if row.get("outcome") == "inconclusive"),
        "confirmed_improvements": sum(
            1 for row in rows if row.get("improvement_outcome") == "confirmed_improvement"
        ),
    }
    if exit_code == 2:
        quality = "invalid_contract"
    elif report["method"]["discovery_only"]:
        quality = "discovery_only"
    elif report["method"]["evidence_mode"] == "diagnostic":
        quality = "diagnostic_advisory"
    elif exit_code == 3:
        quality = "statistically_inconclusive"
    else:
        quality = "decision_grade"
    report["method"]["evidence_quality"] = quality
    return exit_code


def write_decision_markdown(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    summary = report["summary"]
    method = report["method"]
    relative_threshold = method.get("relative_threshold_percent")
    relative_threshold_text = (
        f"+{relative_threshold:.2f}%"
        if isinstance(relative_threshold, (int, float))
        else "unavailable"
    )
    confidence_contract = method.get("confidence_contract", {})
    simultaneous_confidence = confidence_contract.get(
        "simultaneous_confidence_level",
        method.get("confidence_level"),
    )
    component_confidence = confidence_contract.get("component_confidence_level")
    simultaneous_confidence_text = (
        f"{float(simultaneous_confidence) * 100.0:g}%"
        if isinstance(simultaneous_confidence, (int, float))
        and not isinstance(simultaneous_confidence, bool)
        else "unavailable"
    )
    component_confidence_text = (
        f"{float(component_confidence) * 100.0:g}%"
        if isinstance(component_confidence, (int, float))
        and not isinstance(component_confidence, bool)
        else "unavailable"
    )
    lines = [
        "# Merman Self Performance Comparison",
        "",
        "> Generated by `tools/bench/compare_self.py` from schema v2 evidence.",
        "",
        "## Summary",
        "",
        f"- Outcome: `{summary['outcome']}`",
        f"- Exit code: `{summary['exit_code']}`",
        f"- Evidence quality: `{method['evidence_quality']}`",
        f"- Comparable benchmarks: `{summary['comparable']}`",
        f"- Evidence mode: `{method['evidence_mode']}`",
        f"- Preset: `{method['preset']}`",
        f"- Base label: `{report['comparison']['base_label']}`",
        f"- Head label: `{report['comparison']['head_label']}`",
        f"- Relative threshold: `{relative_threshold_text}`",
        f"- Absolute threshold: `{fmt_time(method.get('absolute_threshold_ns'))}`",
        f"- Simultaneous confidence: `{simultaneous_confidence_text}`",
        f"- Multiplicity adjustment: `{confidence_contract.get('multiplicity_adjustment', 'unavailable')}`",
        f"- Confidence family: `{confidence_contract.get('family_size', 'unavailable')}` components "
        f"at `{component_confidence_text}` each",
        "",
        "## Recipes",
        "",
    ]
    for side in ("base", "head"):
        recipe = report.get("recipes", {}).get(side, {})
        runner = report.get("runners", {}).get(side, {})
        git = runner.get("git", {})
        executable = runner.get("executable", {})
        lines.extend(
            [
                f"- {side}: `{recipe.get('package', 'unavailable')}` / "
                f"`{recipe.get('bench', 'unavailable')}`, "
                f"features `{','.join(recipe.get('features', [])) or '<none>'}`, "
                f"logical operations `{recipe.get('logical_operations', 'unavailable')}`, "
                f"revision `{git.get('revision', 'unknown')}`",
                f"- {side} executable SHA-256: `{executable.get('sha256', 'unavailable')}`",
            ]
        )
    lines.extend(
        [
            "",
            "## Results",
            "",
            "| base benchmark | head benchmark | base | head | relative bounds | absolute bounds | outcome | improvement |",
            "|---|---|---:|---:|---:|---:|---|---|",
        ]
    )
    for row in report.get("rows", []):
        bounds = row.get("bounds") or {}
        relative = bounds.get("relative_percent")
        absolute = bounds.get("absolute_ns")
        relative_text = (
            f"{relative['lower']:+.2f}% .. {relative['upper']:+.2f}%"
            if relative
            else "-"
        )
        absolute_text = (
            f"{pretty_time(absolute['lower'])} .. {pretty_time(absolute['upper'])}"
            if absolute
            else "-"
        )
        lines.append(
            f"| `{row.get('base_benchmark') or row['benchmark']}` | "
            f"`{row.get('head_benchmark') or row['benchmark']}` | "
            f"{fmt_time(row.get('base_ns'))} | "
            f"{fmt_time(row.get('head_ns'))} | {relative_text} | {absolute_text} | "
            f"`{row['outcome']}` | `{row.get('improvement_outcome') or '-'}` |"
        )
    if report.get("contract_errors"):
        lines.extend(["", "## Contract Errors", ""])
        for error in report["contract_errors"]:
            lines.append(f"- `{error['stage']}`: {error['message']}")
    lines.extend(
        [
            "",
            "## Interpretation",
            "",
            "Diagnostic results are advisory. Confirmation results use fresh balanced pairs, "
            "paired one-sided confidence bounds, and both registered thresholds. Candidate "
            "admission additionally requires the mirrored improvement result and non-performance gates.",
            "",
        ]
    )
    path.write_text("\n".join(lines), encoding="utf-8")


def _validate_args(args: argparse.Namespace) -> None:
    if args.evidence_mode == "diagnostic" and not 1 <= args.pairs <= 4:
        raise ContractViolation("diagnostic --pairs must be between one and four")
    if args.calibration_pairs < 8 or args.calibration_pairs % 2 != 0:
        raise ContractViolation("--calibration-pairs must be even and at least eight")
    if args.max_pairs < 8 or args.max_pairs % 2 != 0:
        raise ContractViolation("--max-pairs must be even and at least eight")
    if not math.isfinite(args.relative_threshold_percent) or args.relative_threshold_percent <= 0.0:
        raise ContractViolation("--relative-threshold-percent must be positive")
    if args.absolute_threshold_ns is not None and (
        not math.isfinite(args.absolute_threshold_ns) or args.absolute_threshold_ns <= 0.0
    ):
        raise ContractViolation("--absolute-threshold-ns must be positive")
    if args.absolute_threshold_us is not None and (
        not math.isfinite(args.absolute_threshold_us) or args.absolute_threshold_us <= 0.0
    ):
        raise ContractViolation("--absolute-threshold-us must be positive")
    if not math.isclose(args.confidence_level, 0.95, rel_tol=0.0, abs_tol=1e-12):
        raise ContractViolation("the decision contract fixes --confidence-level at 0.95")
    if args.bootstrap_resamples <= 0:
        raise ContractViolation("--bootstrap-resamples must be positive")
    if args.bootstrap_resamples > MAX_BOOTSTRAP_RESAMPLES:
        raise ContractViolation(
            "--bootstrap-resamples must be at most "
            f"{MAX_BOOTSTRAP_RESAMPLES}"
        )
    if (
        args.evidence_mode == "confirmation"
        and args.bootstrap_resamples < DECISION_GRADE_BOOTSTRAP_RESAMPLES
    ):
        raise ContractViolation(
            "confirmation --bootstrap-resamples must be at least "
            f"{DECISION_GRADE_BOOTSTRAP_RESAMPLES}"
        )
    for side in ("base", "head"):
        divisor = getattr(args, f"{side}_logical_operations")
        if divisor is not None and divisor <= 0:
            raise ContractViolation("logical operation counts must be positive")
    if args.timeout_seconds <= 0:
        raise ContractViolation("--timeout-seconds must be positive")
    sample_size, warm_up, measurement = benchmark_params(
        args.preset, args.sample_size, args.warm_up, args.measurement
    )
    if sample_size < 10:
        raise ContractViolation("Criterion --sample-size must be at least ten")
    if warm_up <= 0 or measurement <= 0:
        raise ContractViolation("warm-up and measurement durations must be positive")
    for name in ("base_package", "head_package", "base_bench", "head_bench"):
        if not getattr(args, name).strip():
            raise ContractViolation(f"--{name.replace('_', '-')} must not be empty")


def _validate_confirmation_operation_contract(
    contracts: Sequence[dict[str, Any]],
    *,
    base_logical_operations: int,
    head_logical_operations: int,
    base_lane: LaneMetadata | None = None,
    head_lane: LaneMetadata | None = None,
) -> None:
    if base_lane is None or head_lane is None:
        mismatches = [
            (contract.get("base_benchmark"), contract.get("head_benchmark"))
            for contract in contracts
            if contract.get("base_benchmark") != contract.get("head_benchmark")
        ]
        if mismatches:
            rendered = ", ".join(
                f"{base!r} != {head!r}" for base, head in mismatches[:4]
            )
            raise ContractViolation(
                "confirmation requires the same public operation on both sides; "
                f"mismatched benchmark identities: {rendered}"
            )
        if base_logical_operations != 1 or head_logical_operations != 1:
            raise ContractViolation(
                "confirmation logical-operation divisors must both be one unless corpus "
                "lane metadata owns both normalizations"
            )
        return

    semantic_fields = (
        "kind",
        "owner",
        "public_operation",
        "process_lifecycle",
        "engine_lifecycle",
        "transport",
        "workload",
        "measurement_metrics",
        "semantic_output_dimensions",
    )
    differences = [
        field
        for field in semantic_fields
        if getattr(base_lane, field) != getattr(head_lane, field)
    ]
    if base_lane.kind != "public" or head_lane.kind != "public" or differences:
        raise ContractViolation(
            "confirmation lane metadata does not describe the same public operation: "
            f"differences={differences}"
        )
    divisor_mismatches = []
    if base_logical_operations != base_lane.logical_operations_per_estimate:
        divisor_mismatches.append("base")
    if head_logical_operations != head_lane.logical_operations_per_estimate:
        divisor_mismatches.append("head")
    if divisor_mismatches:
        raise ContractViolation(
            "logical-operation normalization differs from corpus lane metadata: "
            + ", ".join(divisor_mismatches)
        )

    unmatched_groups: list[str] = []
    for contract in contracts:
        for side, lane in (("base", base_lane), ("head", head_lane)):
            exact = contract.get(f"{side}_benchmark")
            if not exact:
                continue
            group, _fixture = split_exact_bench(exact)
            if not _lane_matches_group(lane, group):
                unmatched_groups.append(f"{side}:{group}")
    if unmatched_groups:
        raise ContractViolation(
            "confirmation benchmark groups are outside their lane selector/history contract: "
            + ", ".join(sorted(set(unmatched_groups)))
        )


def _resolve_output_paths(
    *,
    head_dir: Path,
    markdown_value: str,
    json_value: str,
) -> tuple[Path, Path]:
    markdown_path = Path(markdown_value)
    json_path = Path(json_value)
    if not markdown_path.is_absolute():
        markdown_path = head_dir / markdown_path
    if not json_path.is_absolute():
        json_path = head_dir / json_path
    markdown_path = markdown_path.resolve()
    json_path = json_path.resolve()
    if markdown_path == json_path or _same_existing_file(markdown_path, json_path):
        raise ContractViolation(
            "--out and --json-out must resolve to distinct files so Markdown cannot overwrite "
            "the JSON audit evidence"
        )
    return markdown_path, json_path


def _same_existing_file(first: Path, second: Path) -> bool:
    if not first.exists() or not second.exists():
        return False
    try:
        return os.path.samefile(first, second)
    except OSError as error:
        raise ContractViolation(
            f"failed to verify performance evidence output identity: {error}"
        ) from error


def _parse_features(value: str) -> tuple[str, ...]:
    return tuple(dict.fromkeys(item.strip() for item in value.split(",") if item.strip()))


def _absolute_threshold_ns(args: argparse.Namespace) -> float:
    if args.absolute_threshold_ns is not None:
        return float(args.absolute_threshold_ns)
    if args.absolute_threshold_us is not None:
        return float(args.absolute_threshold_us) * 1_000.0
    return 50_000.0


def _build_recipe(args: argparse.Namespace, side: str) -> RunnerRecipe:
    checkout = Path(getattr(args, f"{side}_dir")).resolve()
    target_value = getattr(args, f"{side}_target_dir")
    target_dir = Path(target_value).resolve() if target_value else checkout / "target"
    corpus_value = getattr(args, f"{side}_corpus")
    return RunnerRecipe(
        label=side,
        checkout=checkout,
        package=getattr(args, f"{side}_package"),
        bench=getattr(args, f"{side}_bench"),
        features=_parse_features(getattr(args, f"{side}_features")),
        default_features=getattr(args, f"{side}_default_features"),
        toolchain=getattr(args, f"{side}_toolchain") or None,
        target_dir=target_dir,
        locked=True,
        corpus=Path(corpus_value),
        target=getattr(args, f"{side}_target") or None,
        logical_operations=getattr(args, f"{side}_logical_operations") or 1,
    )


def _empty_report(args: argparse.Namespace, absolute_threshold_ns: float) -> dict[str, Any]:
    sample_size, warm_up, measurement = benchmark_params(
        args.preset, args.sample_size, args.warm_up, args.measurement
    )
    relative_threshold_log = (
        math.log1p(args.relative_threshold_percent / 100.0)
        if math.isfinite(args.relative_threshold_percent)
        and args.relative_threshold_percent > 0.0
        else None
    )
    relative_threshold_percent = (
        args.relative_threshold_percent
        if math.isfinite(args.relative_threshold_percent)
        else None
    )
    report_absolute_threshold_ns = (
        absolute_threshold_ns
        if math.isfinite(absolute_threshold_ns) and absolute_threshold_ns > 0.0
        else None
    )
    return {
        "schema_version": 2,
        "generated_at": dt.datetime.now(dt.timezone.utc).astimezone().isoformat(),
        "comparison": {"base_label": args.base_label, "head_label": args.head_label},
        "selection": {"suite": args.suite},
        "method": {
            "preset": args.preset,
            "evidence_mode": args.evidence_mode,
            "evidence_quality": "pending",
            "discovery_only": args.discovery_only,
            "sample_size": sample_size,
            "warm_up_seconds": warm_up,
            "measurement_seconds": measurement,
            "diagnostic_pairs": args.pairs,
            "calibration_pairs": args.calibration_pairs,
            "max_pairs": args.max_pairs,
            "pair_count": 0,
            "scheduled_pairs": 0,
            "required_pairs": 8,
            "start_side": args.start_side,
            "relative_threshold_percent": relative_threshold_percent,
            "relative_threshold_log": relative_threshold_log,
            "absolute_threshold_ns": report_absolute_threshold_ns,
            "absolute_threshold_us": (
                report_absolute_threshold_ns / 1_000.0
                if report_absolute_threshold_ns is not None
                else None
            ),
            "confidence_level": (
                args.confidence_level if math.isfinite(args.confidence_level) else None
            ),
            "confidence_contract": {
                "simultaneous_confidence_level": (
                    args.confidence_level if math.isfinite(args.confidence_level) else None
                ),
                "component_confidence_level": None,
                "family_size": None,
                "multiplicity_adjustment": "bonferroni",
                "components": "two metrics per comparable benchmark",
            },
            "bootstrap_seed": args.bootstrap_seed,
            "bootstrap_resamples": args.bootstrap_resamples,
            "interval_contract": {
                "confirmation": (
                    "deterministic paired bootstrap Bonferroni simultaneous one-sided bounds"
                ),
                "calibration": (
                    "deterministic paired bootstrap two-sided equivalence intervals"
                ),
            },
        },
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "cpu": best_effort_cpu_model(),
            "python": platform.python_version(),
            "rust": rustc_verbose(),
        },
        "recipes": {},
        "runners": {},
        "fixtures": [],
        "calibration": None,
        "raw_rounds": [],
        "rows": [],
        "contract_errors": [],
    }


def _execute_comparison(args: argparse.Namespace, report: dict[str, Any]) -> None:
    sample_size = report["method"]["sample_size"]
    warm_up = report["method"]["warm_up_seconds"]
    measurement = report["method"]["measurement_seconds"]
    relative_threshold = report["method"]["relative_threshold_log"]
    absolute_threshold_ns = report["method"]["absolute_threshold_ns"]
    base_recipe = _build_recipe(args, "base")
    head_recipe = _build_recipe(args, "head")
    report["recipes"] = {
        "base": _recipe_report(base_recipe),
        "head": _recipe_report(head_recipe),
    }
    if base_recipe.target_dir.resolve() == head_recipe.target_dir.resolve():
        raise ContractViolation("base and head must use distinct Cargo target directories")

    contracts, selection, operation_lanes = _load_fixture_contracts(
        base_recipe=base_recipe,
        head_recipe=head_recipe,
        suite=args.suite,
        common_group=args.group,
        base_group_override=args.base_group,
        head_group_override=args.head_group,
        filter_expr=args.filter,
    )
    base_recipe = _recipe_with_lane_divisor(
        base_recipe,
        lane=operation_lanes[0],
        explicit_divisor=args.base_logical_operations,
    )
    head_recipe = _recipe_with_lane_divisor(
        head_recipe,
        lane=operation_lanes[1],
        explicit_divisor=args.head_logical_operations,
    )
    report["recipes"] = {
        "base": _recipe_report(base_recipe),
        "head": _recipe_report(head_recipe),
    }
    report["selection"] = selection
    report["fixtures"] = contracts
    if not contracts:
        raise ContractViolation("no benchmark fixtures were selected")
    if args.evidence_mode == "confirmation":
        _validate_confirmation_operation_contract(
            contracts,
            base_logical_operations=base_recipe.logical_operations,
            head_logical_operations=head_recipe.logical_operations,
            base_lane=operation_lanes[0],
            head_lane=operation_lanes[1],
        )
    for side in ("base", "head"):
        groups = {
            split_exact_bench(item[f"{side}_benchmark"])[0]
            for item in contracts
            if item[f"{side}_benchmark"]
        }
        if len(groups) > 1:
            raise ContractViolation(
                f"{side} selection spans multiple groups {sorted(groups)}; the runner-level "
                "logical-operations value is only a single-group uniform override"
            )

    prepared: dict[str, PreparedRunner] = {}
    for recipe in (base_recipe, head_recipe):
        runner, provenance, errors = _prepare_runner(
            recipe,
            allow_dirty=args.allow_dirty,
            timeout_seconds=args.timeout_seconds,
        )
        report["runners"][recipe.label] = provenance
        for error in errors:
            _append_contract_error(report, f"prepare_{recipe.label}", error)
        if runner is not None:
            prepared[recipe.label] = runner
    if len(prepared) != 2:
        return
    base = prepared["base"]
    head = prepared["head"]
    if base.executable.resolve() == head.executable.resolve():
        _append_contract_error(
            report,
            "recipe",
            "base and head resolved to the same executable path; frozen candidates are not independent",
        )
        return
    if args.evidence_mode == "confirmation":
        for side, runner in prepared.items():
            if runner.provenance["git"]["dirty"]:
                _append_contract_error(
                    report,
                    f"prepare_{side}",
                    "confirmation evidence requires a clean checkout",
                )
    contracts = _complete_fixture_contracts(contracts, base=base, head=head)
    report["fixtures"] = contracts
    comparable = [item for item in contracts if item["coverage_status"] == "comparable"]
    confidence_family_size = max(2, 2 * len(comparable))
    component_confidence_level = (
        1.0 - (1.0 - args.confidence_level) / confidence_family_size
    )
    report["method"]["confidence_contract"] = {
        "simultaneous_confidence_level": args.confidence_level,
        "component_confidence_level": component_confidence_level,
        "family_size": confidence_family_size,
        "multiplicity_adjustment": "bonferroni",
        "components": "two metrics per comparable benchmark",
    }
    if not comparable:
        _append_contract_error(report, "coverage", "no comparable benchmark rows")
    if args.discovery_only:
        report["rows"] = [
            _comparison_row(
                item,
                pairs=[],
                evidence_mode="diagnostic",
                required_pairs=8,
                relative_threshold=relative_threshold,
                absolute_threshold_ns=absolute_threshold_ns,
                confidence_level=args.confidence_level,
                bootstrap_seed=_stable_seed(args.bootstrap_seed, "discovery", item["name"]),
                bootstrap_resamples=args.bootstrap_resamples,
                confidence_family_size=confidence_family_size,
                forced_outcome=(
                    "diagnostic_advisory"
                    if item["coverage_status"] == "comparable"
                    else "contract_failure"
                ),
                reason="discovery only" if item["coverage_status"] == "comparable" else "; ".join(item["coverage_reasons"]),
            )
            for item in contracts
        ]
        for runner in prepared.values():
            for error in _verification_errors(runner, timeout_seconds=args.timeout_seconds):
                _append_contract_error(report, "freeze_verification", error)
        for error in _fixture_verification_errors(contracts):
            _append_contract_error(report, "freeze_verification", error)
        return

    if report["contract_errors"]:
        report["rows"] = [
            _comparison_row(
                item,
                pairs=[],
                evidence_mode=args.evidence_mode,
                required_pairs=8,
                relative_threshold=relative_threshold,
                absolute_threshold_ns=absolute_threshold_ns,
                confidence_level=args.confidence_level,
                bootstrap_seed=0,
                bootstrap_resamples=args.bootstrap_resamples,
                confidence_family_size=confidence_family_size,
                forced_outcome="contract_failure",
                reason="sampling did not start because the evidence contract was already invalid",
            )
            for item in contracts
        ]
        return

    if args.evidence_mode == "diagnostic":
        report["method"]["scheduled_pairs"] = args.pairs
        schedule = _run_ab_schedule(
            base=base,
            head=head,
            contracts=comparable,
            pair_count=args.pairs,
            start_side=args.start_side,
            sample_size=sample_size,
            warm_up_seconds=warm_up,
            measurement_seconds=measurement,
            timeout_seconds=args.timeout_seconds,
        )
        report["method"]["pair_count"] = min(
            (len(schedule["rows"][item["name"]]) for item in comparable),
            default=0,
        )
        report["raw_rounds"] = schedule["rounds"]
        report["rows"] = []
        for item in contracts:
            if item["coverage_status"] != "comparable":
                row = _comparison_row(
                    item,
                    pairs=[],
                    evidence_mode="diagnostic",
                    required_pairs=8,
                    relative_threshold=relative_threshold,
                    absolute_threshold_ns=absolute_threshold_ns,
                    confidence_level=args.confidence_level,
                    bootstrap_seed=0,
                    bootstrap_resamples=args.bootstrap_resamples,
                    confidence_family_size=confidence_family_size,
                    forced_outcome="contract_failure",
                    reason="; ".join(item["coverage_reasons"]),
                )
            elif item["name"] in schedule["errors"]:
                row = _comparison_row(
                    item,
                    pairs=schedule["rows"][item["name"]],
                    evidence_mode="diagnostic",
                    required_pairs=8,
                    relative_threshold=relative_threshold,
                    absolute_threshold_ns=absolute_threshold_ns,
                    confidence_level=args.confidence_level,
                    bootstrap_seed=0,
                    bootstrap_resamples=args.bootstrap_resamples,
                    confidence_family_size=confidence_family_size,
                    forced_outcome="contract_failure",
                    reason=schedule["errors"][item["name"]],
                )
            else:
                row = _comparison_row(
                    item,
                    pairs=schedule["rows"][item["name"]],
                    evidence_mode="diagnostic",
                    required_pairs=8,
                    relative_threshold=relative_threshold,
                    absolute_threshold_ns=absolute_threshold_ns,
                    confidence_level=args.confidence_level,
                    bootstrap_seed=_stable_seed(args.bootstrap_seed, "diagnostic", item["name"]),
                    bootstrap_resamples=args.bootstrap_resamples,
                    confidence_family_size=confidence_family_size,
                )
            report["rows"].append(row)
    else:
        base_aa = _run_aa_schedule(
            base,
            contracts=comparable,
            pair_count=args.calibration_pairs,
            sample_size=sample_size,
            warm_up_seconds=warm_up,
            measurement_seconds=measurement,
            timeout_seconds=args.timeout_seconds,
        )
        head_aa = _run_aa_schedule(
            head,
            contracts=comparable,
            pair_count=args.calibration_pairs,
            sample_size=sample_size,
            warm_up_seconds=warm_up,
            measurement_seconds=measurement,
            timeout_seconds=args.timeout_seconds,
        )
        calibration: dict[str, Any] = {
            "base": {"rounds": base_aa["rounds"], "rows": {}, "errors": base_aa["errors"]},
            "head": {"rounds": head_aa["rounds"], "rows": {}, "errors": head_aa["errors"]},
        }
        required_by_row: dict[str, int] = {}
        stable_rows: list[dict[str, Any]] = []
        for item in comparable:
            name = item["name"]
            for side, aa in (("base", base_aa), ("head", head_aa)):
                calibration[side]["rows"][name] = _summarize_calibration(
                    aa["rows"][name],
                    expected_pairs=args.calibration_pairs,
                    relative_mde=relative_threshold,
                    absolute_mde_ns=absolute_threshold_ns,
                    max_pairs=args.max_pairs,
                    confidence_level=args.confidence_level,
                    bootstrap_seed=_stable_seed(args.bootstrap_seed, "calibration", side, name),
                    bootstrap_resamples=args.bootstrap_resamples,
                    confidence_family_size=confidence_family_size,
                )
            side_summaries = [calibration[side]["rows"][name] for side in ("base", "head")]
            available_required = [
                summary.get("required_pairs", {}).get("maximum", 8)
                for summary in side_summaries
            ]
            required_by_row[name] = max(available_required)
            if all(summary.get("stable", False) for summary in side_summaries):
                stable_rows.append(item)
        for side, aa in (("base", base_aa), ("head", head_aa)):
            for name, error in aa["errors"].items():
                _append_contract_error(report, f"calibration_{side}:{name}", error)
        suite_required = max(required_by_row.values(), default=8)
        scheduled_pairs = min(suite_required, args.max_pairs)
        report["method"]["required_pairs"] = suite_required
        report["method"]["scheduled_pairs"] = scheduled_pairs
        calibration["suite"] = {
            "required_pairs": suite_required,
            "scheduled_pairs": scheduled_pairs,
            "max_pairs": args.max_pairs,
            "exceeds_cap": suite_required > args.max_pairs,
        }
        report["calibration"] = calibration
        if stable_rows and not report["contract_errors"]:
            schedule = _run_ab_schedule(
                base=base,
                head=head,
                contracts=stable_rows,
                pair_count=scheduled_pairs,
                start_side=args.start_side,
                sample_size=sample_size,
                warm_up_seconds=warm_up,
                measurement_seconds=measurement,
                timeout_seconds=args.timeout_seconds,
            )
        else:
            schedule = {
                "pair_count": scheduled_pairs,
                "rounds": [],
                "rows": {item["name"]: [] for item in stable_rows},
                "errors": {},
            }
        report["method"]["pair_count"] = min(
            (len(schedule["rows"].get(item["name"], [])) for item in comparable),
            default=0,
        )
        report["raw_rounds"] = schedule["rounds"]
        report["rows"] = []
        for item in contracts:
            name = item["name"]
            required = required_by_row.get(name, 8)
            forced: str | None = None
            reason: str | None = None
            pairs = schedule["rows"].get(name, [])
            if item["coverage_status"] != "comparable":
                forced = "contract_failure"
                reason = "; ".join(item["coverage_reasons"])
            elif name in base_aa["errors"] or name in head_aa["errors"]:
                forced = "contract_failure"
                reason = base_aa["errors"].get(name) or head_aa["errors"].get(name)
            elif report["contract_errors"]:
                forced = "contract_failure"
                reason = (
                    "confirmation sampling was globally blocked because another A/A runner "
                    "observation failed the evidence contract"
                )
            elif name in schedule["errors"]:
                forced = "contract_failure"
                reason = schedule["errors"][name]
            elif item not in stable_rows:
                forced = "inconclusive"
                reasons: list[str] = []
                for side in ("base", "head"):
                    reasons.extend(calibration[side]["rows"][name].get("reasons", []))
                reason = "A/A calibration was not stable: " + ", ".join(dict.fromkeys(reasons))
            report["rows"].append(
                _comparison_row(
                    item,
                    pairs=pairs,
                    evidence_mode="confirmation",
                    required_pairs=required,
                    relative_threshold=relative_threshold,
                    absolute_threshold_ns=absolute_threshold_ns,
                    confidence_level=args.confidence_level,
                    bootstrap_seed=_stable_seed(args.bootstrap_seed, "confirmation", name),
                    bootstrap_resamples=args.bootstrap_resamples,
                    confidence_family_size=confidence_family_size,
                    forced_outcome=forced,
                    reason=reason,
                )
            )

    for runner in prepared.values():
        for error in _verification_errors(runner, timeout_seconds=args.timeout_seconds):
            _append_contract_error(report, "freeze_verification", error)
    for error in _fixture_verification_errors(contracts):
        _append_contract_error(report, "freeze_verification", error)


def decision_grade_main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Produce paired decision-grade Merman Criterion evidence between two checkouts."
    )
    parser.add_argument("--base-dir", required=True)
    parser.add_argument("--head-dir", required=True)
    parser.add_argument("--base-label", default="base")
    parser.add_argument("--head-label", default="head")
    parser.add_argument("--base-target-dir", default="")
    parser.add_argument("--head-target-dir", default="")
    parser.add_argument("--base-package", default="merman")
    parser.add_argument("--head-package", default="merman")
    parser.add_argument("--base-bench", default="pipeline")
    parser.add_argument("--head-bench", default="pipeline")
    parser.add_argument("--base-features", default="svg")
    parser.add_argument("--head-features", default="svg")
    parser.add_argument(
        "--base-default-features", action=argparse.BooleanOptionalAction, default=True
    )
    parser.add_argument(
        "--head-default-features", action=argparse.BooleanOptionalAction, default=True
    )
    parser.add_argument("--base-toolchain", default="")
    parser.add_argument("--head-toolchain", default="")
    parser.add_argument("--base-target", default="")
    parser.add_argument("--head-target", default="")
    parser.add_argument("--base-corpus", default=DEFAULT_CORPUS)
    parser.add_argument("--head-corpus", default=DEFAULT_CORPUS)
    parser.add_argument("--suite", default="canary")
    parser.add_argument("--group", default=None)
    parser.add_argument("--base-group", default=None)
    parser.add_argument("--head-group", default=None)
    parser.add_argument("--filter", default=None)
    parser.add_argument("--preset", choices=["quick", "long"], default="quick")
    parser.add_argument("--sample-size", type=int, default=None)
    parser.add_argument("--warm-up", type=int, default=None)
    parser.add_argument("--measurement", type=int, default=None)
    parser.add_argument(
        "--evidence-mode", choices=["diagnostic", "confirmation"], default="diagnostic"
    )
    parser.add_argument("--pairs", type=int, default=2, help="Diagnostic AB/BA pair count (1-4).")
    parser.add_argument("--calibration-pairs", type=int, default=8)
    parser.add_argument("--max-pairs", type=int, default=32)
    parser.add_argument("--start-side", choices=["base", "head"], default="base")
    parser.add_argument("--relative-threshold-percent", type=float, default=10.0)
    absolute = parser.add_mutually_exclusive_group()
    absolute.add_argument("--absolute-threshold-ns", type=float, default=None)
    absolute.add_argument("--absolute-threshold-us", type=float, default=None)
    parser.add_argument("--confidence-level", type=float, default=0.95)
    parser.add_argument("--bootstrap-seed", type=int, default=0)
    parser.add_argument(
        "--bootstrap-resamples",
        type=int,
        default=DECISION_GRADE_BOOTSTRAP_RESAMPLES,
    )
    parser.add_argument(
        "--base-logical-operations",
        type=int,
        default=None,
        help=(
            "Legacy divisor override for a group without lane metadata; registered lanes "
            "must match their corpus-owned divisor."
        ),
    )
    parser.add_argument(
        "--head-logical-operations",
        type=int,
        default=None,
        help=(
            "Legacy divisor override for a group without lane metadata; registered lanes "
            "must match their corpus-owned divisor."
        ),
    )
    parser.add_argument("--timeout-seconds", type=int, default=1_800)
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--discovery-only", action="store_true")
    parser.add_argument("--out", default=DEFAULT_MARKDOWN_OUT)
    parser.add_argument("--json-out", default=DEFAULT_JSON_OUT)
    args = parser.parse_args(argv)

    head_dir = Path(args.head_dir).resolve()
    try:
        markdown_path, json_path = _resolve_output_paths(
            head_dir=head_dir,
            markdown_value=args.out,
            json_value=args.json_out,
        )
    except ContractViolation as error:
        print(f"invalid performance evidence outputs: {error}", file=sys.stderr)
        return 2

    absolute_threshold_ns = _absolute_threshold_ns(args)
    report = _empty_report(args, absolute_threshold_ns)
    try:
        _validate_args(args)
        _execute_comparison(args, report)
    except KeyboardInterrupt:
        raise
    except Exception as error:
        _append_contract_error(report, "orchestration", str(error))

    exit_code = _finalize_summary(report)
    try:
        write_json_report(json_path, report)
    except Exception as error:
        print(f"failed to write JSON performance evidence: {error}", file=sys.stderr)
        return 2
    print(f"Wrote: {json_path}")
    try:
        outputs_collide = _same_existing_file(markdown_path, json_path)
    except ContractViolation as error:
        outputs_collide = True
        collision_message = str(error)
    else:
        collision_message = (
            "Markdown output resolves to the JSON audit file after JSON creation"
            if outputs_collide
            else ""
        )
    if outputs_collide:
        _append_contract_error(report, "report_projection", collision_message)
        _finalize_summary(report)
        try:
            write_json_report(json_path, report)
        except Exception as error:
            print(f"failed to preserve JSON performance evidence: {error}", file=sys.stderr)
        print(f"refused to overwrite JSON performance evidence: {collision_message}", file=sys.stderr)
        return 2
    try:
        write_decision_markdown(markdown_path, report)
    except Exception as error:
        _append_contract_error(report, "report_projection", str(error))
        _finalize_summary(report)
        write_json_report(json_path, report)
        print(f"failed to write Markdown performance evidence: {error}", file=sys.stderr)
        return 2
    print(f"Wrote: {markdown_path}")
    print(f"Outcome: {report['summary']['outcome']} (exit {exit_code})")
    return exit_code


def main(argv: list[str]) -> int:
    return decision_grade_main(argv)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

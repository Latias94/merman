#!/usr/bin/env python3
"""Run the isolated native allocator evidence lane and write a complete audit report."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import math
import os
import platform
import secrets
import subprocess
import sys
from collections.abc import Callable, Mapping, Sequence
from pathlib import Path, PurePosixPath
from typing import Any

from compare_self import RunnerRecipe, cargo_prebuild_command, parse_bench_executable
from compare_mermaid_renderers import best_effort_cpu_model
from corpus_utils import LaneMetadata, load_corpus, resolve_lane_selector
from native_memory import (
    MAX_BOOTSTRAP_RESAMPLES,
    MEMORY_SCALES,
    MIN_REPEATS,
    MemoryContractError,
    classify_memory_metric,
    paired_adjustments,
    suite_exit_code,
    validate_response,
    validate_sample_matrix,
    validate_semantic_response_contract,
)


DEFAULT_CORPUS = Path("tools/bench/corpus.json")
DEFAULT_LANE = "flowchart-end-to-end-memory"
DEFAULT_WORKLOAD = "flowchart-modular-generator-v1"
DEFAULT_REPORT = Path("target/bench/native_memory.json")
DEFAULT_REPEATS = 5
DEFAULT_SEED = 0x4D45524D414E
DEFAULT_BOOTSTRAP_RESAMPLES = 10_000
DEFAULT_TIMEOUT_SECONDS = 300
_METRICS = ("allocation_count", "allocated_bytes", "peak_growth_bytes")
_OWNER_CONTRACT_COMMON_FIELDS = frozenset(
    {
        "schema_version",
        "lane_id",
        "workload",
        "evidence_class",
        "candidate_admission",
        "metrics",
    }
)
_OWNER_CONTRACT_V1_FIELDS = _OWNER_CONTRACT_COMMON_FIELDS | frozenset({"generator"})
_OWNER_CONTRACT_V2_FIELDS = _OWNER_CONTRACT_COMMON_FIELDS | frozenset(
    {"probe", "scale", "semantic_response"}
)
_GENERATOR_FIELDS = frozenset({"id", "nodes_per_scale", "edges_per_scale"})
_PROBE_FIELDS = frozenset(
    {
        "protocol_schema_version",
        "package",
        "package_manifest",
        "bench",
        "features",
        "default_features",
    }
)
_SCALE_FIELDS = frozenset({"dimension", "units_per_scale"})
_BOUND_FIELDS = frozenset({"slope_cap", "max_scale_cap"})
_BUILD_ENVIRONMENT_OVERRIDES = {
    "CARGO_BUILD_JOBS": "1",
    "CARGO_INCREMENTAL": "0",
    "CARGO_PROFILE_BENCH_DEBUG": "0",
}


class DriverContractError(ValueError):
    """The driver cannot produce trustworthy native-memory evidence."""


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _describe_file(path: Path, *, root: Path) -> dict[str, object]:
    try:
        display_path = path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        display_path = str(path.resolve())
    return {
        "path": display_path,
        "bytes": path.stat().st_size,
        "sha256": _sha256_path(path),
    }


def _git_provenance(root: Path) -> dict[str, object]:
    def git(*arguments: str) -> str:
        result = subprocess.run(
            ["git", *arguments],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise DriverContractError(
                f"git {' '.join(arguments)} failed: {result.stderr[-1000:]}"
            )
        return result.stdout

    commit = git("rev-parse", "--verify", "HEAD").strip()
    tree = git("rev-parse", "HEAD^{tree}").strip()
    status = git("status", "--porcelain=v1", "--untracked-files=all")
    return {
        "commit": commit,
        "tree": tree,
        "clean": not status,
        "dirty_status_sha256": hashlib.sha256(status.encode("utf-8")).hexdigest(),
        "dirty_disposition": "clean" if not status else "unapproved",
    }


def _build_environment() -> dict[str, str | None]:
    return {
        **_BUILD_ENVIRONMENT_OVERRIDES,
        **{
            key: os.environ.get(key)
            for key in (
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "RUSTUP_TOOLCHAIN",
                "CARGO_PROFILE_BENCH_LTO",
                "CARGO_PROFILE_BENCH_CODEGEN_UNITS",
                "CARGO_PROFILE_BENCH_OPT_LEVEL",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
            )
        },
    }


def _verify_source_unchanged(
    initial: Mapping[str, object], current: Mapping[str, object]
) -> None:
    fields = ("commit", "tree", "clean", "dirty_status_sha256")
    changed = [field for field in fields if initial.get(field) != current.get(field)]
    if changed:
        raise DriverContractError(
            "Git source disposition changed during native-memory sampling: "
            + ", ".join(changed)
        )


def _toolchain_command(toolchain: str | None, executable: str, *args: str) -> list[str]:
    if toolchain:
        return ["rustup", "run", toolchain, executable, *args]
    return [executable, *args]


def _strict_json_object(path: Path) -> dict[str, object]:
    def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise DriverContractError(f"duplicate JSON key in {path}: {key}")
            result[key] = value
        return result

    def reject_constant(token: str) -> None:
        raise DriverContractError(f"non-finite JSON number in {path}: {token}")

    try:
        value = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicates,
            parse_constant=reject_constant,
        )
    except DriverContractError:
        raise
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise DriverContractError(f"cannot read owner contract {path}: {error}") from error
    if not isinstance(value, dict):
        raise DriverContractError(f"owner contract must be a JSON object: {path}")
    return value


def _positive_number(value: object, *, field: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise DriverContractError(f"{field} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number <= 0.0:
        raise DriverContractError(f"{field} must be finite and positive")
    return number


def _positive_int(value: object, *, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise DriverContractError(f"{field} must be a positive integer")
    return value


def _nonempty_string(value: object, *, field: str) -> str:
    if not isinstance(value, str) or not value.strip() or value != value.strip():
        raise DriverContractError(f"{field} must be a trimmed non-empty string")
    return value


def _lowercase_sha256(value: object, *, field: str) -> str:
    if (
        not isinstance(value, str)
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise DriverContractError(f"{field} must be a lowercase SHA-256 digest")
    return value


def _repo_relative_path(value: object, *, field: str) -> str:
    path = _nonempty_string(value, field=field)
    pure = PurePosixPath(path)
    if (
        pure.is_absolute()
        or path != pure.as_posix()
        or any(part in ("", ".", "..") for part in pure.parts)
        or "\\" in path
    ):
        raise DriverContractError(f"{field} must be a normalized repo-relative path")
    return path


def _resolve_repo_file(root: Path, relative: str, *, field: str) -> Path:
    try:
        resolved_root = root.resolve(strict=True)
        resolved = (resolved_root / relative).resolve(strict=True)
        resolved.relative_to(resolved_root)
    except (OSError, ValueError) as error:
        raise DriverContractError(f"{field} does not resolve to a repository file") from error
    if not resolved.is_file():
        raise DriverContractError(f"{field} does not resolve to a repository file")
    return resolved


def _validate_metrics_contract(contract: Mapping[str, object]) -> None:
    metrics = contract["metrics"]
    if not isinstance(metrics, dict) or frozenset(metrics) != frozenset(_METRICS):
        raise DriverContractError("owner contract must define exactly three allocator metrics")
    for metric in _METRICS:
        bounds = metrics[metric]
        if not isinstance(bounds, dict) or frozenset(bounds) != _BOUND_FIELDS:
            raise DriverContractError(f"owner contract bounds differ for {metric}")
        _positive_number(bounds["slope_cap"], field=f"{metric}.slope_cap")
        _positive_number(bounds["max_scale_cap"], field=f"{metric}.max_scale_cap")


def load_owner_contract(path: Path, *, lane: LaneMetadata) -> dict[str, object]:
    contract = _strict_json_object(path)
    schema_version = contract.get("schema_version")
    if isinstance(schema_version, bool) or schema_version not in (1, 2):
        raise DriverContractError("unsupported owner contract schema_version")
    expected_fields = (
        _OWNER_CONTRACT_V1_FIELDS
        if schema_version == 1
        else _OWNER_CONTRACT_V2_FIELDS
    )
    if frozenset(contract) != expected_fields:
        raise DriverContractError(
            f"owner contract fields differ from schema version {schema_version}"
        )
    if contract["lane_id"] != lane.id or contract["workload"] != lane.workload:
        raise DriverContractError("owner contract identity differs from the selected lane")
    evidence_class = contract["evidence_class"]
    candidate_admission = contract["candidate_admission"]
    valid_evidence_identity = (
        evidence_class == "infrastructure-smoke" and candidate_admission is False
    ) or (evidence_class == "candidate-bound" and candidate_admission is True)
    if not valid_evidence_identity:
        raise DriverContractError(
            "owner contract evidence class and candidate-admission flag disagree"
        )

    if schema_version == 1:
        generator = contract["generator"]
        if not isinstance(generator, dict) or frozenset(generator) != _GENERATOR_FIELDS:
            raise DriverContractError("owner contract generator fields differ")
        if generator["id"] != lane.workload:
            raise DriverContractError("owner contract generator id differs from lane workload")
        _positive_int(generator["nodes_per_scale"], field="generator.nodes_per_scale")
        _positive_int(generator["edges_per_scale"], field="generator.edges_per_scale")
    else:
        probe = contract["probe"]
        if not isinstance(probe, dict) or frozenset(probe) != _PROBE_FIELDS:
            raise DriverContractError("owner contract probe fields differ")
        if probe["protocol_schema_version"] != 2:
            raise DriverContractError("schema-v2 owner probe must use protocol schema 2")
        package = _nonempty_string(probe["package"], field="probe.package")
        if package != lane.owner:
            raise DriverContractError("owner contract probe package differs from lane owner")
        _repo_relative_path(probe["package_manifest"], field="probe.package_manifest")
        _nonempty_string(probe["bench"], field="probe.bench")
        if not isinstance(probe["default_features"], bool):
            raise DriverContractError("probe.default_features must be a boolean")
        features = probe["features"]
        if (
            not isinstance(features, list)
            or any(not isinstance(value, str) or not value for value in features)
            or len(features) != len(set(features))
            or tuple(features) != lane.required_features
        ):
            raise DriverContractError(
                "owner contract probe features differ from lane required_features"
            )
        scale = contract["scale"]
        if not isinstance(scale, dict) or frozenset(scale) != _SCALE_FIELDS:
            raise DriverContractError("owner contract scale fields differ")
        dimension = _nonempty_string(scale["dimension"], field="scale.dimension")
        _positive_int(scale["units_per_scale"], field="scale.units_per_scale")
        semantic_response = contract["semantic_response"]
        if not isinstance(semantic_response, dict):
            raise DriverContractError("semantic_response must be an object")
        try:
            scale_field = validate_semantic_response_contract(
                semantic_response,
                semantic_output_dimensions=lane.semantic_output_dimensions,
            )
        except MemoryContractError as error:
            raise DriverContractError(str(error)) from error
        if dimension != scale_field:
            raise DriverContractError(
                "owner scale dimension differs from semantic response scale_field"
            )

    _validate_metrics_contract(contract)
    return contract


def memory_recipe(
    root: Path,
    *,
    target_dir: Path,
    toolchain: str | None,
    corpus: Path = DEFAULT_CORPUS,
    contract: Mapping[str, object] | None = None,
) -> RunnerRecipe:
    if contract is not None and contract.get("schema_version") == 2:
        probe = contract["probe"]
        assert isinstance(probe, Mapping)
        package = str(probe["package"])
        bench = str(probe["bench"])
        features = tuple(str(value) for value in probe["features"])  # type: ignore[union-attr]
        default_features = bool(probe["default_features"])
    else:
        package = "merman"
        bench = "native_memory"
        features = ("svg",)
        default_features = False
    return RunnerRecipe(
        label="native-memory",
        checkout=root,
        package=package,
        bench=bench,
        features=features,
        default_features=default_features,
        toolchain=toolchain,
        target_dir=target_dir,
        locked=True,
        corpus=corpus,
    )


def discover_executable(
    recipe: RunnerRecipe,
    *,
    timeout_seconds: int,
) -> tuple[Path, dict[str, object]]:
    command = cargo_prebuild_command(recipe)
    environment = os.environ.copy()
    environment.update(_BUILD_ENVIRONMENT_OVERRIDES)
    try:
        result = subprocess.run(
            command,
            cwd=recipe.checkout,
            env=environment,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise DriverContractError("native-memory locked prebuild timed out") from error
    if result.returncode != 0:
        raise DriverContractError(
            "native-memory locked prebuild failed: " + result.stderr[-2_000:]
        )
    try:
        executable = parse_bench_executable(result.stdout, recipe=recipe)
    except ValueError as error:
        raise DriverContractError(str(error)) from error
    if not executable.is_absolute():
        executable = (recipe.checkout / executable).resolve()
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise DriverContractError(f"Cargo reported an unusable executable: {executable}")
    return executable, {
        "command": command,
        "stdout_sha256": hashlib.sha256(result.stdout.encode("utf-8")).hexdigest(),
        "stderr_sha256": hashlib.sha256(result.stderr.encode("utf-8")).hexdigest(),
    }


def build_schedule(
    *,
    lane_id: str = DEFAULT_LANE,
    repeats: int,
    seed: int,
    run_id: str,
    nonce_factory: Callable[[], str] = lambda: secrets.token_hex(16),
) -> list[dict[str, object]]:
    _validate_schedule_parameters(repeats=repeats, seed=seed, run_id=run_id)
    schedule: list[dict[str, object]] = []
    nonces: set[str] = set()
    pair_index = 0
    for scale in MEMORY_SCALES:
        for repeat in range(repeats):
            order = ("operation", "zero") if pair_index % 2 == 0 else ("zero", "operation")
            for position, mode in enumerate(order):
                nonce = nonce_factory()
                if nonce in nonces:
                    raise DriverContractError("nonce factory produced a repeated identity")
                nonces.add(nonce)
                schedule.append(
                    {
                        "pair_index": pair_index,
                        "position": position,
                        "request": {
                            "schema_version": 1,
                            "lane_id": lane_id,
                            "mode": mode,
                            "scale": scale,
                            "seed": seed,
                            "repeat": repeat,
                            "invocation_id": f"{run_id}:{scale}:{repeat}:{mode}",
                            "nonce": nonce,
                        },
                    }
                )
            pair_index += 1
    return schedule


def boundary_smoke_schedule(
    schedule: Sequence[dict[str, object]],
) -> list[dict[str, object]]:
    """Select repeat zero at both registered boundary scales."""

    boundaries = {MEMORY_SCALES[0], MEMORY_SCALES[-1]}
    selected: list[dict[str, object]] = []
    for entry in schedule:
        request = entry.get("request")
        if (
            isinstance(request, Mapping)
            and request.get("scale") in boundaries
            and request.get("repeat") == 0
        ):
            selected.append(entry)
    if len(selected) != 4:
        raise DriverContractError(
            "native-memory smoke requires one operation/zero pair at each boundary scale"
        )
    return selected


def _validate_schedule_parameters(*, repeats: int, seed: int, run_id: str) -> None:
    if repeats < MIN_REPEATS:
        raise DriverContractError(f"at least {MIN_REPEATS} repeats are required")
    if repeats > 2**32:
        raise DriverContractError("repeat indexes must fit the native u32 protocol")
    if seed < 0 or seed > 2**64 - 1:
        raise DriverContractError("seed must fit the native u64 protocol")
    if not run_id or run_id != run_id.strip() or len(run_id) > 192:
        raise DriverContractError(
            "run id must be a trimmed non-empty string of at most 192 characters"
        )


def _validate_driver_parameters(args: argparse.Namespace) -> None:
    _validate_schedule_parameters(
        repeats=args.repeats,
        seed=args.seed,
        run_id=args.run_id or "generated-run-id",
    )
    if args.bootstrap_resamples < DEFAULT_BOOTSTRAP_RESAMPLES:
        raise DriverContractError(
            f"decision evidence requires at least {DEFAULT_BOOTSTRAP_RESAMPLES} resamples"
        )
    if args.bootstrap_resamples > MAX_BOOTSTRAP_RESAMPLES:
        raise DriverContractError(
            f"bootstrap resamples must be at most {MAX_BOOTSTRAP_RESAMPLES}"
        )
    if args.timeout_seconds <= 0:
        raise DriverContractError("timeout seconds must be positive")


def _expected_echo(
    request: Mapping[str, object],
    *,
    executable_sha256: str,
    lane: LaneMetadata,
) -> dict[str, object]:
    return {
        "lane_id": request["lane_id"],
        "public_operation": lane.public_operation,
        "process_lifecycle": lane.process_lifecycle,
        "engine_lifecycle": lane.engine_lifecycle,
        "logical_operations_per_estimate": lane.logical_operations_per_estimate,
        "mode": request["mode"],
        "scale": request["scale"],
        "seed": request["seed"],
        "repeat": request["repeat"],
        "executable_sha256": executable_sha256,
        "invocation_id": request["invocation_id"],
        "nonce": request["nonce"],
    }


def run_probe(
    executable: Path,
    request: Mapping[str, object],
    *,
    executable_sha256: str,
    lane: LaneMetadata,
    timeout_seconds: int,
    generator: Mapping[str, object] | None = None,
    contract: Mapping[str, object] | None = None,
) -> dict[str, object]:
    request_line = json.dumps(request, separators=(",", ":"), allow_nan=False) + "\n"
    try:
        result = subprocess.run(
            [str(executable)],
            input=request_line,
            capture_output=True,
            text=True,
            timeout=timeout_seconds,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise DriverContractError(
            f"native-memory subprocess timed out for {request['invocation_id']}"
        ) from error
    if result.returncode != 0:
        raise DriverContractError(
            f"native-memory subprocess exited {result.returncode} for "
            f"{request['invocation_id']}: {result.stdout[-1_000:]}"
        )
    semantic_contract: Mapping[str, object] | None = None
    workload_units_per_scale: int | None = None
    contract_schema = contract.get("schema_version") if contract is not None else 1
    if contract_schema == 2:
        raw_semantic_contract = contract["semantic_response"]  # type: ignore[index]
        scale_contract = contract["scale"]  # type: ignore[index]
        assert isinstance(raw_semantic_contract, Mapping)
        assert isinstance(scale_contract, Mapping)
        semantic_contract = raw_semantic_contract
        workload_units_per_scale = int(scale_contract["units_per_scale"])
    elif generator is None:
        if contract is None or not isinstance(contract.get("generator"), Mapping):
            raise DriverContractError("schema-v1 probe requires a generator contract")
        generator = contract["generator"]  # type: ignore[assignment]

    try:
        response = validate_response(
            result.stdout,
            result.stderr,
            expected=_expected_echo(
                request,
                executable_sha256=executable_sha256,
                lane=lane,
            ),
            semantic_contract=semantic_contract,
            workload_units_per_scale=workload_units_per_scale,
        )
    except MemoryContractError as error:
        raise DriverContractError(str(error)) from error

    if contract_schema == 1:
        assert generator is not None
        scale = int(request["scale"])
        expected_nodes = int(generator["nodes_per_scale"]) * scale
        expected_edges = int(generator["edges_per_scale"]) * scale
        if (
            response["input_nodes"] != expected_nodes
            or response["input_edges"] != expected_edges
        ):
            raise DriverContractError(
                f"generator dimensions differ at scale {scale}: "
                f"expected {expected_nodes}/{expected_edges}, got "
                f"{response['input_nodes']}/{response['input_edges']}"
            )
    return response


def analyze_samples(
    samples: Sequence[Mapping[str, object]],
    *,
    contract: Mapping[str, object],
    bootstrap_resamples: int,
    seed_material: str,
) -> tuple[dict[str, object], list[str]]:
    try:
        matrix = validate_sample_matrix(samples)
    except MemoryContractError as error:
        raise DriverContractError(str(error)) from error
    if not matrix["complete"]:
        raise DriverContractError(
            "native-memory matrix is incomplete: "
            + "; ".join(matrix["incomplete_reasons"])  # type: ignore[arg-type]
        )

    metric_contracts = contract["metrics"]
    assert isinstance(metric_contracts, Mapping)
    metrics: dict[str, object] = {}
    outcomes: list[str] = []
    for metric in _METRICS:
        bounds = metric_contracts[metric]
        assert isinstance(bounds, Mapping)
        try:
            adjustments = paired_adjustments(samples, metric=metric)
            result = classify_memory_metric(
                adjustments,
                slope_cap=float(bounds["slope_cap"]),
                max_scale_cap=float(bounds["max_scale_cap"]),
                seed_material=f"{seed_material}:{metric}",
                resamples=bootstrap_resamples,
            )
        except MemoryContractError as error:
            raise DriverContractError(str(error)) from error
        metrics[metric] = {"adjustments": adjustments, **result}
        outcomes.append(str(result["outcome"]))
    return {"matrix": matrix, "metrics": metrics}, outcomes


def _atomic_json(path: Path, value: Mapping[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.parent / f".{path.name}.{os.getpid()}.tmp"
    payload = json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n"
    try:
        temporary.write_text(payload, encoding="utf-8")
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def _tool_output(command: list[str], *, root: Path) -> str:
    try:
        result = subprocess.run(
            command,
            cwd=root,
            capture_output=True,
            text=True,
            timeout=30,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return "unavailable"
    return result.stdout.strip() if result.returncode == 0 else "unavailable"


def _base_report(args: argparse.Namespace, *, output: Path) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).astimezone().isoformat(),
        "outcome": "contract_failure",
        "exit_code": 2,
        "output": str(output),
        "method": {
            "scales": list(MEMORY_SCALES),
            "repeats": args.repeats,
            "seed": args.seed,
            "bootstrap_resamples": args.bootstrap_resamples,
            "subprocess_isolation": "fresh-process-per-sample",
            "pair_order": "alternating-operation-zero",
            "evidence_class": "protocol-smoke" if args.smoke else None,
        },
        "candidate_admission": False,
        "environment": {
            "os": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "cpu": best_effort_cpu_model(),
            "python": platform.python_version(),
        },
        "contract_errors": [],
        "schedule": [],
        "analysis": None,
    }


def execute(args: argparse.Namespace) -> tuple[dict[str, object], int]:
    root = repo_root()
    output = Path(args.json_out)
    if not output.is_absolute():
        output = root / output
    report = _base_report(args, output=output)
    try:
        _validate_driver_parameters(args)
        source = _git_provenance(root)
        report["source"] = source
        if not source["clean"]:
            if not args.allow_dirty and not args.smoke and not args.dry_run:
                raise DriverContractError(
                    "decision memory evidence requires a clean Git worktree; "
                    "use --allow-dirty only for diagnostic investigation"
                )
            source["dirty_disposition"] = "allowed-diagnostic"
            report["candidate_admission"] = False
        corpus_path = Path(args.corpus)
        if not corpus_path.is_absolute():
            corpus_path = root / corpus_path
        corpus = load_corpus(corpus_path)
        lane = resolve_lane_selector(corpus, args.lane)
        if lane.transport != "native-system-allocator-subprocess":
            raise DriverContractError(f"lane {lane.id!r} is not a native memory lane")
        if lane.process_lifecycle != "fresh-process":
            raise DriverContractError("native memory lane must declare fresh-process isolation")
        if lane.kind != "public" or lane.logical_operations_per_estimate != 1:
            raise DriverContractError(
                "selected lane semantics are unsupported by the native-memory probe"
            )
        contract_path = Path(args.contract or lane.evidence_contract or "")
        if not contract_path.is_absolute():
            contract_path = root / contract_path
        contract = load_owner_contract(contract_path, lane=lane)
        if contract["schema_version"] == 1:
            if (
                lane.id != DEFAULT_LANE
                or lane.workload != DEFAULT_WORKLOAD
                or lane.public_operation != "render-svg"
                or lane.engine_lifecycle != "reused-engine"
            ):
                raise DriverContractError(
                    "selected schema-v1 lane semantics are unsupported by the "
                    "native-memory probe"
                )
        if not args.smoke:
            report["method"]["evidence_class"] = contract["evidence_class"]  # type: ignore[index]
            report["candidate_admission"] = bool(
                contract["candidate_admission"] and source["clean"]
            )
        report["lane"] = {
            "id": lane.id,
            "public_operation": lane.public_operation,
            "process_lifecycle": lane.process_lifecycle,
            "engine_lifecycle": lane.engine_lifecycle,
            "logical_operations_per_estimate": lane.logical_operations_per_estimate,
            "transport": lane.transport,
            "workload": lane.workload,
            "size_vector": list(lane.size_vector),
            "measurement_metrics": list(lane.measurement_metrics),
            "semantic_output_dimensions": list(lane.semantic_output_dimensions),
        }

        target_dir = Path(args.target_dir)
        if not target_dir.is_absolute():
            target_dir = root / target_dir
        target_dir = target_dir.resolve()
        recipe = memory_recipe(
            root,
            target_dir=target_dir,
            toolchain=args.toolchain,
            corpus=corpus_path,
            contract=contract,
        )
        if lane.required_features != recipe.features:
            raise DriverContractError(
                "lane required features differ from the native-memory Cargo recipe"
            )

        if contract["schema_version"] == 2:
            probe = contract["probe"]
            assert isinstance(probe, Mapping)
            package_manifest_path = _resolve_repo_file(
                root,
                str(probe["package_manifest"]),
                field="probe.package_manifest",
            )
        else:
            package_manifest_path = root / "crates" / "merman" / "Cargo.toml"

        report_inputs: dict[str, object] = {
            "workspace_manifest": _describe_file(root / "Cargo.toml", root=root),
            "package_manifest": _describe_file(package_manifest_path, root=root),
            "cargo_lock": _describe_file(root / "Cargo.lock", root=root),
            "corpus": {
                "path": str(corpus_path),
                "bytes": corpus_path.stat().st_size,
                "sha256": _sha256_path(corpus_path),
            },
            "owner_contract": {
                "path": str(contract_path),
                "bytes": contract_path.stat().st_size,
                "sha256": _sha256_path(contract_path),
                "value": contract,
            },
        }
        report["inputs"] = report_inputs
        build_command = cargo_prebuild_command(recipe)
        report["recipe"] = {
            "package": recipe.package,
            "bench": recipe.bench,
            "features": list(recipe.features),
            "default_features": recipe.default_features,
            "locked": recipe.locked,
            "target_dir": str(recipe.target_dir),
            "build_command": build_command,
            "build_environment": _build_environment(),
            "requested_toolchain": args.toolchain,
        }
        if args.dry_run:
            report["dry_run"] = True
            report["planned_subprocesses"] = len(MEMORY_SCALES) * args.repeats * 2
            return report, 0

        if args.executable:
            executable = Path(args.executable).resolve()
            build_provenance: dict[str, object] = {"skipped": "explicit executable"}
        else:
            executable, build_provenance = discover_executable(
                recipe, timeout_seconds=args.timeout_seconds
            )
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise DriverContractError(f"native-memory executable is unusable: {executable}")
        executable_digest = _sha256_path(executable)
        report["executable"] = {
            "path": str(executable),
            "bytes": executable.stat().st_size,
            "sha256": executable_digest,
            "build": build_provenance,
        }
        report["environment"].update(  # type: ignore[union-attr]
            {
                "rustc": _tool_output(
                    _toolchain_command(args.toolchain, "rustc", "-Vv"),
                    root=root,
                ),
                "cargo": _tool_output(
                    _toolchain_command(args.toolchain, "cargo", "-V"),
                    root=root,
                ),
            }
        )

        run_id = args.run_id or (
            dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
            + "-"
            + secrets.token_hex(8)
        )
        schedule = build_schedule(
            lane_id=lane.id,
            repeats=args.repeats,
            seed=args.seed,
            run_id=run_id,
        )
        if args.smoke:
            schedule = boundary_smoke_schedule(schedule)
        report["run_id"] = run_id
        report["schedule"] = schedule
        samples: list[dict[str, object]] = []
        for entry in schedule:
            request = entry["request"]
            assert isinstance(request, Mapping)
            response = run_probe(
                executable,
                request,
                executable_sha256=executable_digest,
                lane=lane,
                contract=contract,
                timeout_seconds=args.timeout_seconds,
            )
            entry["response"] = response
            samples.append(response)
        if _sha256_path(executable) != executable_digest:
            raise DriverContractError("native-memory executable changed during sampling")
        _verify_source_unchanged(source, _git_provenance(root))

        if args.smoke:
            matrix = validate_sample_matrix(samples)
            report.update(
                {
                    "run_id": run_id,
                    "schedule": schedule,
                    "analysis": {"matrix": matrix, "metrics": None},
                    "outcome": "protocol_smoke_pass",
                    "exit_code": 0,
                }
            )
            return report, 0

        analysis, outcomes = analyze_samples(
            samples,
            contract=contract,
            bootstrap_resamples=args.bootstrap_resamples,
            seed_material=f"{lane.id}:{args.seed}:{args.repeats}",
        )
        exit_code = suite_exit_code(outcomes)
        outcome = (
            "failed_bound"
            if exit_code == 1
            else "inconclusive" if exit_code == 3 else "pass"
        )
        report.update(
            {
                "run_id": run_id,
                "schedule": schedule,
                "analysis": analysis,
                "outcome": outcome,
                "exit_code": exit_code,
            }
        )
        return report, exit_code
    except Exception as error:
        report["contract_errors"].append(str(error))  # type: ignore[union-attr]
        return report, 2


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description="Run isolated native System allocator evidence for one registered lane."
    )
    parser.add_argument("--corpus", default=str(DEFAULT_CORPUS))
    parser.add_argument("--lane", default=DEFAULT_LANE)
    parser.add_argument("--contract", default="")
    parser.add_argument("--executable", default="")
    parser.add_argument("--target-dir", default="target")
    parser.add_argument("--toolchain", default=None)
    parser.add_argument("--repeats", type=int, default=DEFAULT_REPEATS)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument(
        "--bootstrap-resamples", type=int, default=DEFAULT_BOOTSTRAP_RESAMPLES
    )
    parser.add_argument("--timeout-seconds", type=int, default=DEFAULT_TIMEOUT_SECONDS)
    parser.add_argument("--run-id", default="")
    parser.add_argument("--json-out", default=str(DEFAULT_REPORT))
    parser.add_argument(
        "--allow-dirty",
        action="store_true",
        help="Allow explicitly diagnostic evidence from a dirty worktree.",
    )
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument(
        "--smoke",
        action="store_true",
        help="Run real operation/zero pairs at the 1x and 100x boundaries without decision bounds.",
    )
    mode.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    report, exit_code = execute(args)
    if args.dry_run:
        recipe = report.get("recipe", {})
        command = recipe.get("build_command", []) if isinstance(recipe, dict) else []
        if command:
            print("$ " + " ".join(str(part) for part in command))
        print(f"planned fresh subprocesses: {report.get('planned_subprocesses', 0)}")
        if exit_code != 0:
            for error in report.get("contract_errors", []):
                print(f"contract failure: {error}", file=sys.stderr)
        return exit_code

    output = Path(args.json_out)
    if not output.is_absolute():
        output = repo_root() / output
    try:
        _atomic_json(output, report)
    except (OSError, TypeError, ValueError) as error:
        print(f"failed to write native-memory evidence: {error}", file=sys.stderr)
        return 2
    print(f"Wrote: {output}")
    print(f"Outcome: {report['outcome']} (exit {exit_code})")
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

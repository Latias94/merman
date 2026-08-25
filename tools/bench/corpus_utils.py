#!/usr/bin/env python3
"""
Shared helpers for the corpus-driven benchmark scripts.
"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from native_memory import MEMORY_SCALES

_LANE_FIELDS = frozenset(
    {
        "id",
        "kind",
        "owner",
        "public_operation",
        "diagnostic_stage",
        "parent_public_lane",
        "process_lifecycle",
        "engine_lifecycle",
        "logical_operations_per_estimate",
        "transport",
        "required_features",
        "selector",
        "history_aliases",
        "size_vector",
        "workload",
        "evidence_contract",
        "measurement_metrics",
        "semantic_output_dimensions",
    }
)
_LANE_KINDS = frozenset({"public", "diagnostic"})
_PROCESS_LIFECYCLES = frozenset({"fresh-process", "reused-process"})
_ENGINE_LIFECYCLES = frozenset(
    {"cold-engine", "reused-engine", "not-applicable"}
)
_TRANSPORTS = frozenset(
    {
        "native-criterion",
        "native-system-allocator-subprocess",
        "node-napi",
        "node-wasm",
        "web-wasm",
        "browser-mermaid-js",
        "native-mermaid-rs-renderer",
    }
)


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
class LaneMetadata:
    id: str
    kind: str
    owner: str
    public_operation: str
    diagnostic_stage: str | None
    parent_public_lane: str | None
    process_lifecycle: str
    engine_lifecycle: str
    logical_operations_per_estimate: int
    transport: str
    required_features: tuple[str, ...]
    selector: str
    history_aliases: tuple[str, ...]
    size_vector: tuple[int, ...]
    workload: str
    evidence_contract: str | None
    measurement_metrics: tuple[str, ...]
    semantic_output_dimensions: tuple[str, ...]


@dataclass(frozen=True)
class Corpus:
    schema_version: int
    default_group: str
    suites: dict[str, str]
    fixtures: tuple[CorpusFixture, ...]
    lanes: tuple[LaneMetadata, ...] = ()


def _required_string(item: dict[str, object], field: str, *, lane: str) -> str:
    value = item.get(field)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"lane {lane}.{field} must be a non-empty string")
    return value


def _optional_string(item: dict[str, object], field: str, *, lane: str) -> str | None:
    value = item.get(field)
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"lane {lane}.{field} must be null or a non-empty string")
    return value


def _string_list(
    item: dict[str, object],
    field: str,
    *,
    lane: str,
    allow_empty: bool,
) -> tuple[str, ...]:
    value = item.get(field)
    if not isinstance(value, list):
        raise ValueError(f"lane {lane}.{field} must be a list")
    result: list[str] = []
    for entry in value:
        if not isinstance(entry, str) or not entry.strip():
            raise ValueError(
                f"lane {lane}.{field} entries must be non-empty strings"
            )
        result.append(entry)
    if not allow_empty and not result:
        raise ValueError(f"lane {lane}.{field} must not be empty")
    if len(result) != len(set(result)):
        raise ValueError(f"lane {lane}.{field} contains duplicate values")
    return tuple(result)


def _load_lanes(data: dict[str, object], schema_version: int) -> tuple[LaneMetadata, ...]:
    if schema_version == 1:
        lanes_raw = data.get("lanes", [])
        if lanes_raw not in (None, []):
            raise ValueError("corpus schema_version 1 cannot declare lanes")
        return ()

    lanes_raw = data.get("lanes")
    if not isinstance(lanes_raw, list):
        raise ValueError("corpus.lanes must be a list for schema_version 2")
    if not lanes_raw:
        raise ValueError("corpus.lanes must not be empty for schema_version 2")

    lanes: list[LaneMetadata] = []
    for index, raw_lane in enumerate(lanes_raw):
        if not isinstance(raw_lane, dict):
            raise ValueError(f"lane entry {index} must be an object")
        fields = frozenset(raw_lane)
        if fields != _LANE_FIELDS:
            missing = sorted(_LANE_FIELDS - fields)
            unknown = sorted(fields - _LANE_FIELDS)
            raise ValueError(
                f"lane entry {index} fields differ: missing={missing}, unknown={unknown}"
            )

        lane_hint = str(raw_lane.get("id") or index)
        lane_id = _required_string(raw_lane, "id", lane=lane_hint)
        kind = _required_string(raw_lane, "kind", lane=lane_id)
        if kind not in _LANE_KINDS:
            raise ValueError(f"lane {lane_id}.kind is not registered: {kind!r}")
        owner = _required_string(raw_lane, "owner", lane=lane_id)
        public_operation = _required_string(
            raw_lane, "public_operation", lane=lane_id
        )
        diagnostic_stage = _optional_string(
            raw_lane, "diagnostic_stage", lane=lane_id
        )
        parent_public_lane = _optional_string(
            raw_lane, "parent_public_lane", lane=lane_id
        )

        process_lifecycle = _required_string(
            raw_lane, "process_lifecycle", lane=lane_id
        )
        if process_lifecycle not in _PROCESS_LIFECYCLES:
            raise ValueError(
                f"lane {lane_id}.process_lifecycle is not registered: "
                f"{process_lifecycle!r}"
            )
        engine_lifecycle = _required_string(
            raw_lane, "engine_lifecycle", lane=lane_id
        )
        if engine_lifecycle not in _ENGINE_LIFECYCLES:
            raise ValueError(
                f"lane {lane_id}.engine_lifecycle is not registered: "
                f"{engine_lifecycle!r}"
            )

        logical_operations = raw_lane.get("logical_operations_per_estimate")
        if (
            isinstance(logical_operations, bool)
            or not isinstance(logical_operations, int)
            or logical_operations <= 0
        ):
            raise ValueError(
                f"lane {lane_id}.logical_operations_per_estimate must be a positive integer"
            )

        transport = _required_string(raw_lane, "transport", lane=lane_id)
        if transport not in _TRANSPORTS:
            raise ValueError(
                f"lane {lane_id}.transport is not registered: {transport!r}"
            )

        required_features = _string_list(
            raw_lane,
            "required_features",
            lane=lane_id,
            allow_empty=True,
        )
        selector = _required_string(raw_lane, "selector", lane=lane_id)
        history_aliases = _string_list(
            raw_lane,
            "history_aliases",
            lane=lane_id,
            allow_empty=True,
        )
        for lane_selector in (selector, *history_aliases):
            group = lane_selector_group(lane_selector)
            if transport == "native-criterion" and "/" in group:
                raise ValueError(
                    f"native Criterion lane {lane_id} group must not contain '/': {group!r}"
                )
        workload = _required_string(raw_lane, "workload", lane=lane_id)
        evidence_contract = _optional_string(
            raw_lane, "evidence_contract", lane=lane_id
        )
        measurement_metrics = _string_list(
            raw_lane,
            "measurement_metrics",
            lane=lane_id,
            allow_empty=False,
        )
        semantic_output_dimensions = _string_list(
            raw_lane,
            "semantic_output_dimensions",
            lane=lane_id,
            allow_empty=False,
        )

        size_vector_raw = raw_lane.get("size_vector")
        if not isinstance(size_vector_raw, list):
            raise ValueError(f"lane {lane_id}.size_vector must be a list")
        size_vector: list[int] = []
        for value in size_vector_raw:
            if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
                raise ValueError(
                    f"lane {lane_id}.size_vector entries must be positive integers"
                )
            size_vector.append(value)
        if len(size_vector) != len(set(size_vector)) or size_vector != sorted(size_vector):
            raise ValueError(
                f"lane {lane_id}.size_vector must be unique and increasing"
            )
        if (
            transport == "native-system-allocator-subprocess"
            and tuple(size_vector) != MEMORY_SCALES
        ):
            raise ValueError(
                f"lane {lane_id}.size_vector must be exactly {MEMORY_SCALES}"
            )
        if transport == "native-system-allocator-subprocess":
            if process_lifecycle != "fresh-process":
                raise ValueError(
                    f"memory lane {lane_id} must use fresh-process isolation"
                )
            if evidence_contract is None:
                raise ValueError(
                    f"memory lane {lane_id} requires an owner evidence contract"
                )
            required_metrics = frozenset(
                {"allocation_count", "allocated_bytes", "peak_growth_bytes"}
            )
            supported_metrics = required_metrics | {"retained_growth_bytes"}
            registered_metrics = frozenset(measurement_metrics)
            if not required_metrics.issubset(
                registered_metrics
            ) or not registered_metrics.issubset(supported_metrics):
                raise ValueError(
                    f"memory lane {lane_id} must register the three native allocator "
                    "metrics and only supported optional metrics"
                )
            if public_operation == "render-svg":
                required_output = {
                    "input_nodes",
                    "input_edges",
                    "svg_sha256",
                    "svg_viewbox_width",
                    "svg_viewbox_height",
                }
                if not required_output.issubset(semantic_output_dimensions):
                    raise ValueError(
                        f"memory lane {lane_id} is missing semantic output evidence"
                    )
        if kind == "public":
            if diagnostic_stage is not None or parent_public_lane is not None:
                raise ValueError(
                    f"public lane {lane_id} cannot declare diagnostic ownership"
                )
        elif diagnostic_stage is None or parent_public_lane is None:
            raise ValueError(
                f"diagnostic lane {lane_id} requires a stage and public parent"
            )

        lanes.append(
            LaneMetadata(
                id=lane_id,
                kind=kind,
                owner=owner,
                public_operation=public_operation,
                diagnostic_stage=diagnostic_stage,
                parent_public_lane=parent_public_lane,
                process_lifecycle=process_lifecycle,
                engine_lifecycle=engine_lifecycle,
                logical_operations_per_estimate=logical_operations,
                transport=transport,
                required_features=required_features,
                selector=selector,
                history_aliases=history_aliases,
                size_vector=tuple(size_vector),
                workload=workload,
                evidence_contract=evidence_contract,
                measurement_metrics=measurement_metrics,
                semantic_output_dimensions=semantic_output_dimensions,
            )
        )

    namespace: dict[str, LaneMetadata] = {}
    lanes_by_id = {lane.id: lane for lane in lanes}
    if len(lanes_by_id) != len(lanes):
        raise ValueError("duplicate lane id in corpus")
    for lane in lanes:
        for selector in (lane.id, lane.selector, *lane.history_aliases):
            prior = namespace.get(selector)
            if prior is not None:
                raise ValueError(
                    f"lane namespace collision {selector!r}: {prior.id} and {lane.id}"
                )
            namespace[selector] = lane

    for lane in lanes:
        if lane.kind != "diagnostic":
            continue
        parent = lanes_by_id.get(lane.parent_public_lane or "")
        if parent is None or parent.kind != "public":
            raise ValueError(
                f"diagnostic lane {lane.id} has no registered public parent"
            )
        if parent.owner != lane.owner:
            raise ValueError(
                f"diagnostic lane {lane.id} and its parent must share one owner"
            )
        if parent.public_operation != lane.public_operation:
            raise ValueError(
                f"diagnostic lane {lane.id} and its parent must share one public operation"
            )
        if (
            parent.process_lifecycle != lane.process_lifecycle
            or parent.engine_lifecycle != lane.engine_lifecycle
            or parent.transport != lane.transport
            or parent.logical_operations_per_estimate
            != lane.logical_operations_per_estimate
            or parent.required_features != lane.required_features
            or parent.size_vector != lane.size_vector
            or parent.workload != lane.workload
            or parent.measurement_metrics != lane.measurement_metrics
        ):
            raise ValueError(
                f"diagnostic lane {lane.id} must share lifecycle, transport, divisor, "
                "features, size vector, and workload with its public parent"
            )

    return tuple(lanes)


def load_corpus(path: Path) -> Corpus:
    def reject_duplicates(pairs: list[tuple[str, object]]) -> dict[str, object]:
        result: dict[str, object] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"duplicate JSON key in corpus: {key}")
            result[key] = value
        return result

    def reject_constant(token: str) -> None:
        raise ValueError(f"non-finite JSON number in corpus: {token}")

    data = json.loads(
        path.read_text(encoding="utf-8"),
        object_pairs_hook=reject_duplicates,
        parse_constant=reject_constant,
    )
    if not isinstance(data, dict):
        raise ValueError(f"corpus must be a JSON object: {path}")
    schema_version_raw = data.get("schema_version", 0)
    if isinstance(schema_version_raw, bool) or not isinstance(schema_version_raw, int):
        raise ValueError("corpus.schema_version must be an integer")
    schema_version = schema_version_raw
    if schema_version not in (1, 2):
        raise ValueError(f"unsupported corpus schema_version: {schema_version}")

    lanes = _load_lanes(data, schema_version)

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
        lanes=lanes,
    )


def resolve_lane_selector(corpus: Corpus, selector: str) -> LaneMetadata:
    """Resolve a current id/selector or a historical selector alias."""

    if not isinstance(selector, str) or not selector:
        raise ValueError("lane selector must be a non-empty string")
    for lane in corpus.lanes:
        if selector in (lane.id, lane.selector, *lane.history_aliases):
            return lane
    raise ValueError(f"unknown lane selector: {selector!r}")


def lane_selector_group(selector: str) -> str:
    """Return the Criterion group owned by a fixture selector pattern."""

    suffix = "/{fixture}"
    if not isinstance(selector, str) or not selector.endswith(suffix):
        raise ValueError(
            f"lane selector must end with {suffix!r}: {selector!r}"
        )
    group = selector[: -len(suffix)]
    if not group:
        raise ValueError(f"lane selector has an invalid Criterion group: {selector!r}")
    return group


def resolve_lane_group(corpus: Corpus, group: str) -> LaneMetadata:
    """Resolve a current or historical Criterion group to its lane contract."""

    if not isinstance(group, str) or not group or "/" in group:
        raise ValueError("lane group must be a non-empty Criterion group")
    matches = [
        lane
        for lane in corpus.lanes
        if any(
            lane_selector_group(selector) == group
            for selector in (lane.selector, *lane.history_aliases)
        )
    ]
    if len(matches) != 1:
        raise ValueError(
            f"lane group {group!r} resolved to {len(matches)} lane contracts"
        )
    return matches[0]


def select_corpus_fixtures(corpus: Corpus, suite: str) -> list[CorpusFixture]:
    if suite == "full":
        return list(corpus.fixtures)
    fixtures = [f for f in corpus.fixtures if suite in f.suites]
    if not fixtures:
        available = ", ".join(sorted(corpus.suites))
        raise ValueError(f"unknown or empty suite {suite!r}; available suites: {available}")
    return fixtures


def fixture_names_for_suite(corpus: Corpus, suite: str) -> tuple[str, ...]:
    return tuple(f.name for f in select_corpus_fixtures(corpus, suite))


def resolve_merman_fixture_path(
    repo_root: Path,
    name: str,
    fixture: CorpusFixture | None = None,
) -> Path:
    candidates: list[Path] = []
    if fixture is not None:
        candidates.append(repo_root / fixture.source)
    candidates.append(repo_root / "crates" / "merman" / "benches" / "fixtures" / f"{name}.mmd")
    return next((path for path in candidates if path.exists()), candidates[0])


def compare_mmdr_fixture_inputs(
    *,
    repo_root: Path,
    mmdr_dir: Path,
    fixture_names: Iterable[str],
    fixtures_by_name: dict[str, CorpusFixture] | None = None,
) -> dict[str, dict[str, object]]:
    comparisons: dict[str, dict[str, object]] = {}
    metadata = fixtures_by_name or {}

    for name in dict.fromkeys(fixture_names):
        merman_path = resolve_merman_fixture_path(repo_root, name, metadata.get(name))
        mmdr_path = mmdr_dir / "benches" / "fixtures" / f"{name}.mmd"

        def describe(path: Path, root: Path) -> dict[str, object]:
            relative = str(path.relative_to(root)).replace("\\", "/")
            if not path.exists():
                return {"path": relative, "bytes": None, "sha256": None}
            data = path.read_bytes()
            return {
                "path": relative,
                "bytes": len(data),
                "sha256": hashlib.sha256(data).hexdigest(),
            }

        merman = describe(merman_path, repo_root)
        mmdr = describe(mmdr_path, mmdr_dir)
        if merman["sha256"] is None and mmdr["sha256"] is None:
            status = "missing_both"
        elif merman["sha256"] is None:
            status = "missing_merman"
        elif mmdr["sha256"] is None:
            status = "missing_mmdr"
        elif merman["sha256"] == mmdr["sha256"]:
            status = "identical"
        else:
            status = "different"

        comparisons[name] = {
            "status": status,
            "merman": merman,
            "mermaid_rs_renderer": mmdr,
        }

    return comparisons

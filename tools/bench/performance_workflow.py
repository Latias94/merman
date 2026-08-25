#!/usr/bin/env python3
"""Select and enforce the structured performance workflow contract."""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TextIO


ROOT = Path(__file__).resolve().parents[2]
DEFAULT_REGISTRY = ROOT / "tools" / "bench" / "performance_lanes.json"

_LANE_FIELDS = (
    "id",
    "pull_request_label",
    "suite",
    "group",
    "package",
    "bench",
    "corpus",
    "features",
    "default_features",
    "artifact",
    "title",
    "marker",
)


class WorkflowContractError(ValueError):
    """The checked-in performance workflow contract is invalid."""


@dataclass(frozen=True)
class LaneRegistry:
    lanes: tuple[dict[str, object], ...]
    scheduled_lanes: tuple[str, ...]
    dispatch_runs: dict[str, tuple[str, ...]]

    @property
    def by_id(self) -> dict[str, dict[str, object]]:
        return {str(lane["id"]): lane for lane in self.lanes}


def _string(value: object, *, field: str, allow_empty: bool = False) -> str:
    if not isinstance(value, str) or (not allow_empty and not value):
        suffix = "string" if allow_empty else "non-empty string"
        raise WorkflowContractError(f"{field} must be a {suffix}")
    return value


def _lane_ids(value: object, *, field: str, known: set[str]) -> tuple[str, ...]:
    if not isinstance(value, list):
        raise WorkflowContractError(f"{field} must be an array")
    lane_ids = tuple(_string(item, field=field) for item in value)
    if len(set(lane_ids)) != len(lane_ids):
        raise WorkflowContractError(f"{field} contains duplicate lane ids")
    unknown = sorted(set(lane_ids) - known)
    if unknown:
        raise WorkflowContractError(f"{field} references unknown lanes: {unknown}")
    return lane_ids


def load_registry(path: Path = DEFAULT_REGISTRY) -> LaneRegistry:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise WorkflowContractError(f"cannot load {path}: {error}") from error
    if not isinstance(payload, dict):
        raise WorkflowContractError("performance lane registry must be an object")
    if payload.get("schema_version") != 1:
        raise WorkflowContractError("performance lane registry schema_version must be 1")

    raw_lanes = payload.get("lanes")
    if not isinstance(raw_lanes, list) or not raw_lanes:
        raise WorkflowContractError("lanes must be a non-empty array")
    lanes: list[dict[str, object]] = []
    seen_ids: set[str] = set()
    seen_labels: set[str] = set()
    for index, raw_lane in enumerate(raw_lanes):
        if not isinstance(raw_lane, dict) or set(raw_lane) != set(_LANE_FIELDS):
            raise WorkflowContractError(
                f"lanes[{index}] fields must be exactly {list(_LANE_FIELDS)}"
            )
        lane = dict(raw_lane)
        for field in _LANE_FIELDS:
            if field == "default_features":
                if not isinstance(lane[field], bool):
                    raise WorkflowContractError(
                        f"lanes[{index}].default_features must be a boolean"
                    )
            elif field == "pull_request_label":
                if lane[field] is not None:
                    lane[field] = _string(
                        lane[field], field=f"lanes[{index}].{field}"
                    )
            else:
                lane[field] = _string(
                    lane[field],
                    field=f"lanes[{index}].{field}",
                    allow_empty=field == "group",
                )
        lane_id = str(lane["id"])
        label = lane["pull_request_label"]
        if lane_id in seen_ids:
            raise WorkflowContractError(f"duplicate lane id: {lane_id}")
        if label is not None and label in seen_labels:
            raise WorkflowContractError(f"duplicate pull-request label: {label}")
        seen_ids.add(lane_id)
        if label is not None:
            seen_labels.add(label)
        lanes.append(lane)

    scheduled_lanes = _lane_ids(
        payload.get("scheduled_lanes"),
        field="scheduled_lanes",
        known=seen_ids,
    )
    raw_dispatch = payload.get("dispatch_runs")
    if not isinstance(raw_dispatch, dict) or not raw_dispatch:
        raise WorkflowContractError("dispatch_runs must be a non-empty object")
    dispatch_runs = {
        _string(name, field="dispatch run name"): _lane_ids(
            lane_ids,
            field=f"dispatch_runs.{name}",
            known=seen_ids,
        )
        for name, lane_ids in raw_dispatch.items()
    }
    return LaneRegistry(tuple(lanes), scheduled_lanes, dispatch_runs)


def parse_labels(raw: str) -> frozenset[str]:
    try:
        value = json.loads(raw or "[]")
    except json.JSONDecodeError as error:
        raise WorkflowContractError(f"labels JSON is invalid: {error}") from error
    if value is None:
        return frozenset()
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise WorkflowContractError("labels JSON must be an array of strings")
    return frozenset(value)


def select_lane_ids(
    registry: LaneRegistry,
    *,
    event_name: str,
    dispatch_run: str = "",
    labels: frozenset[str] = frozenset(),
) -> tuple[str, ...]:
    if event_name == "pull_request":
        return tuple(
            str(lane["id"])
            for lane in registry.lanes
            if lane["pull_request_label"] is not None
            and str(lane["pull_request_label"]) in labels
        )
    if event_name == "schedule":
        return registry.scheduled_lanes
    if event_name == "workflow_dispatch":
        try:
            return registry.dispatch_runs[dispatch_run]
        except KeyError as error:
            raise WorkflowContractError(
                f"unsupported performance lane: {dispatch_run or '<empty>'}"
            ) from error
    raise WorkflowContractError(f"unsupported performance event: {event_name}")


def measurement_matrix(
    registry: LaneRegistry,
    *,
    event_name: str,
    dispatch_run: str = "",
    labels: frozenset[str] = frozenset(),
) -> dict[str, list[dict[str, object]]]:
    selected = select_lane_ids(
        registry,
        event_name=event_name,
        dispatch_run=dispatch_run,
        labels=labels,
    )
    lanes = registry.by_id
    include = []
    for lane_id in selected:
        descriptor = dict(lanes[lane_id])
        descriptor.pop("pull_request_label")
        include.append(descriptor)
    return {"include": include}


def write_matrix_outputs(
    output: TextIO,
    registry: LaneRegistry,
    *,
    event_name: str,
    dispatch_run: str = "",
    labels: frozenset[str] = frozenset(),
) -> None:
    matrix = measurement_matrix(
        registry,
        event_name=event_name,
        dispatch_run=dispatch_run,
        labels=labels,
    )
    output.write(f"matrix={json.dumps(matrix, separators=(',', ':'))}\n")
    output.write(f"selected={'true' if matrix['include'] else 'false'}\n")


def enforce_measurement_result(
    *,
    comparison_exit: int,
    render_exit: int,
    error: TextIO,
) -> int:
    if render_exit != 0:
        print(
            "::error::The performance report consumer rejected the evidence contract.",
            file=error,
        )
        return 2
    messages = {
        0: None,
        1: "::error::Decision-grade evidence confirmed a performance regression.",
        2: "::error::The performance comparison evidence contract failed.",
        3: "::error::The performance comparison was statistically inconclusive.",
    }
    if comparison_exit not in messages:
        print(
            f"::error::Unexpected performance comparison exit code: {comparison_exit}",
            file=error,
        )
        return 2
    message = messages[comparison_exit]
    if message is not None:
        print(message, file=error)
    return comparison_exit


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    select = subparsers.add_parser("select", help="write a GitHub Actions matrix")
    select.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    select.add_argument("--event", required=True)
    select.add_argument("--dispatch-run", default="")
    select.add_argument("--labels-json", default="[]")
    select.add_argument("--github-output", type=Path, required=True)

    enforce = subparsers.add_parser("enforce", help="enforce producer outcomes")
    enforce.add_argument("--comparison-exit", type=int, required=True)
    enforce.add_argument("--render-exit", type=int, required=True)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    try:
        if args.command == "select":
            registry = load_registry(args.registry)
            labels = parse_labels(args.labels_json)
            args.github_output.parent.mkdir(parents=True, exist_ok=True)
            with args.github_output.open("a", encoding="utf-8") as output:
                write_matrix_outputs(
                    output,
                    registry,
                    event_name=args.event,
                    dispatch_run=args.dispatch_run,
                    labels=labels,
                )
            return 0
        return enforce_measurement_result(
            comparison_exit=args.comparison_exit,
            render_exit=args.render_exit,
            error=sys.stderr,
        )
    except WorkflowContractError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())

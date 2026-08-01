"""Dependency-free structural reader for GitHub Actions contract checks.

The reader intentionally supports the block-style subset used by this repository. Security
verifiers need jobs, steps, permissions, environments, and environment variables, but should not
silently accept YAML aliases, flow mappings, duplicate keys, or other shapes they do not model.
"""

from __future__ import annotations

from pathlib import Path
from typing import Any


class WorkflowContractError(ValueError):
    pass


def load_workflow_contract(path: Path) -> dict[str, Any]:
    lines = path.read_text(encoding="utf-8").splitlines()
    jobs_markers = _mapping_lines(lines, "jobs", indent=0)
    if len(jobs_markers) != 1:
        raise WorkflowContractError(f"{path}: expected exactly one top-level jobs mapping")
    jobs_start = jobs_markers[0]

    document: dict[str, Any] = {
        "header": "\n".join(lines[:jobs_start]),
        "permissions": {},
        "permissions_declared": False,
        "jobs": {},
    }
    permission_markers = _mapping_lines(lines[:jobs_start], "permissions", indent=0)
    if len(permission_markers) > 1:
        raise WorkflowContractError(f"{path}: duplicate top-level permissions mapping")
    if permission_markers:
        permissions_end = _next_at_most_indent(lines, permission_markers[0] + 1, jobs_start, 0)
        document["permissions"] = _flat_mapping(
            lines,
            permission_markers[0] + 1,
            permissions_end,
            2,
            owner=f"{path}:permissions",
        )
        document["permissions_declared"] = True

    jobs: dict[str, Any] = {}
    job_starts = [
        (index, _scalar(line.strip()[:-1]))
        for index, line in enumerate(lines[jobs_start + 1 :], start=jobs_start + 1)
        if _indent(line) == 2 and line.strip().endswith(":") and not _comment(line)
    ]
    for position, (start, job_id) in enumerate(job_starts):
        if job_id in jobs:
            raise WorkflowContractError(f"{path}: duplicate job {job_id!r}")
        end = job_starts[position + 1][0] if position + 1 < len(job_starts) else len(lines)
        jobs[job_id] = _parse_job(lines, start + 1, end, owner=f"{path}:{job_id}")
    document["jobs"] = jobs
    return document


def workflow_job(document: dict[str, Any], job_id: str) -> dict[str, Any]:
    jobs = document.get("jobs")
    if not isinstance(jobs, dict) or job_id not in jobs:
        raise WorkflowContractError(f"workflow job not found: {job_id}")
    job = jobs[job_id]
    if not isinstance(job, dict):
        raise WorkflowContractError(f"workflow job has an invalid shape: {job_id}")
    return job


def workflow_step(
    job: dict[str, Any],
    *,
    name: str | None = None,
    step_id: str | None = None,
) -> dict[str, Any]:
    if (name is None) == (step_id is None):
        raise ValueError("select a workflow step by exactly one of name or step_id")
    steps = job.get("steps")
    if not isinstance(steps, list):
        raise WorkflowContractError("workflow job does not contain a step list")
    for step in steps:
        if name is not None and step.get("name") == name:
            return step
        if step_id is not None and step.get("id") == step_id:
            return step
    wanted = f"name={name!r}" if name is not None else f"id={step_id!r}"
    raise WorkflowContractError(f"workflow step not found: {wanted}")


def _parse_job(lines: list[str], start: int, end: int, *, owner: str) -> dict[str, Any]:
    job: dict[str, Any] = {
        "env": {},
        "permissions": {},
        "permissions_declared": False,
        "steps": [],
    }
    seen_keys: set[str] = set()
    for index in range(start, end):
        line = lines[index]
        if _indent(line) != 4 or _comment(line) or ":" not in line:
            continue
        key, raw_value = line.strip().split(":", 1)
        key = _scalar(key)
        if key in seen_keys:
            raise WorkflowContractError(f"{owner}: duplicate job key {key!r}")
        seen_keys.add(key)
        raw_value = raw_value.strip()
        if key in {"env", "outputs", "permissions"}:
            if raw_value:
                raise WorkflowContractError(f"{owner}: {key} must be a block mapping")
            block_end = _next_at_most_indent(lines, index + 1, end, 4)
            job[key] = _flat_mapping(
                lines,
                index + 1,
                block_end,
                6,
                owner=f"{owner}:{key}",
            )
            if key == "permissions":
                job["permissions_declared"] = True
        elif key == "needs" and not raw_value:
            block_end = _next_at_most_indent(lines, index + 1, end, 4)
            job[key] = _scalar_list(
                lines,
                index + 1,
                block_end,
                6,
                owner=f"{owner}:{key}",
            )
        elif key == "steps":
            if raw_value:
                raise WorkflowContractError(f"{owner}: steps must be a block list")
            job["steps"] = _parse_steps(lines, index + 1, end, owner=owner)
        elif key == "strategy" and not raw_value:
            block_end = _next_at_most_indent(lines, index + 1, end, 4)
            matrix_include = _parse_matrix_include(
                lines,
                index + 1,
                block_end,
                owner=f"{owner}:strategy",
            )
            if matrix_include is not None:
                job["matrix_include"] = matrix_include
        elif key == "environment" and not raw_value:
            block_end = _next_at_most_indent(lines, index + 1, end, 4)
            job[key] = _flat_mapping(
                lines,
                index + 1,
                block_end,
                6,
                owner=f"{owner}:environment",
            )
        elif raw_value:
            job[key] = _scalar(raw_value)
    return job


def _parse_steps(lines: list[str], start: int, end: int, *, owner: str) -> list[dict[str, Any]]:
    starts = [
        index
        for index in range(start, end)
        if _indent(lines[index]) == 6 and lines[index].lstrip().startswith("- ")
    ]
    steps: list[dict[str, Any]] = []
    for position, step_start in enumerate(starts):
        step_end = starts[position + 1] if position + 1 < len(starts) else end
        step_owner = f"{owner}:steps[{position}]"
        step: dict[str, Any] = {"env": {}, "with": {}}
        first = lines[step_start].lstrip()[2:]
        entries = [(step_start, first)] + [
            (index, lines[index].strip())
            for index in range(step_start + 1, step_end)
            if _indent(lines[index]) == 8 and not _comment(lines[index]) and ":" in lines[index]
        ]
        seen_keys: set[str] = set()
        for index, entry in entries:
            if ":" not in entry:
                raise WorkflowContractError(f"{step_owner}: expected a mapping entry")
            key, raw_value = entry.split(":", 1)
            key = _scalar(key)
            if key in seen_keys:
                raise WorkflowContractError(f"{step_owner}: duplicate step key {key!r}")
            seen_keys.add(key)
            raw_value = raw_value.strip()
            _reject_unmodeled_scalar(raw_value, f"{step_owner}:{key}")
            if key in {"env", "with"}:
                if raw_value:
                    raise WorkflowContractError(f"{step_owner}: {key} must be a block mapping")
                block_end = _next_at_most_indent(lines, index + 1, step_end, 8)
                step[key] = _flat_mapping(
                    lines,
                    index + 1,
                    block_end,
                    10,
                    owner=f"{step_owner}:{key}",
                )
            elif key == "run" and _is_block_scalar(raw_value):
                block_end = _next_at_most_indent(lines, index + 1, step_end, 8)
                step[key] = _block_scalar_text(lines[index + 1 : block_end])
            elif raw_value:
                step[key] = _scalar(raw_value)
        has_uses = isinstance(step.get("uses"), str)
        has_run = isinstance(step.get("run"), str)
        if has_uses == has_run:
            raise WorkflowContractError(f"{step_owner}: step must define exactly one of uses or run")
        steps.append(step)
    return steps


def _parse_matrix_include(
    lines: list[str],
    start: int,
    end: int,
    *,
    owner: str,
) -> list[dict[str, str]] | None:
    matrix_markers = [
        index
        for index in range(start, end)
        if _indent(lines[index]) == 6 and lines[index].strip() == "matrix:"
    ]
    if not matrix_markers:
        return None
    if len(matrix_markers) != 1:
        raise WorkflowContractError(f"{owner}: duplicate matrix mapping")

    matrix_start = matrix_markers[0]
    matrix_end = _next_at_most_indent(lines, matrix_start + 1, end, 6)
    include_markers = [
        index
        for index in range(matrix_start + 1, matrix_end)
        if _indent(lines[index]) == 8 and lines[index].strip() == "include:"
    ]
    if not include_markers:
        return None
    if len(include_markers) != 1:
        raise WorkflowContractError(f"{owner}: duplicate matrix include list")

    include_start = include_markers[0]
    include_end = _next_at_most_indent(lines, include_start + 1, matrix_end, 8)
    row_starts = [
        index
        for index in range(include_start + 1, include_end)
        if _indent(lines[index]) == 10 and lines[index].lstrip().startswith("- ")
    ]
    if not row_starts:
        raise WorkflowContractError(f"{owner}: matrix include must contain at least one row")

    rows: list[dict[str, str]] = []
    for position, row_start in enumerate(row_starts):
        row_end = row_starts[position + 1] if position + 1 < len(row_starts) else include_end
        row_owner = f"{owner}:matrix.include[{position}]"
        entries = [(row_start, lines[row_start].lstrip()[2:])] + [
            (index, lines[index].strip())
            for index in range(row_start + 1, row_end)
            if _indent(lines[index]) == 12 and not _comment(lines[index])
        ]
        row: dict[str, str] = {}
        for _index, entry in entries:
            if ":" not in entry:
                raise WorkflowContractError(f"{row_owner}: expected a mapping entry")
            key, raw_value = entry.split(":", 1)
            key = _scalar(key)
            raw_value = raw_value.strip()
            if key in row:
                raise WorkflowContractError(f"{row_owner}: duplicate row key {key!r}")
            _reject_unmodeled_scalar(raw_value, f"{row_owner}:{key}")
            if not raw_value:
                raise WorkflowContractError(f"{row_owner}: nested row values are unsupported")
            row[key] = _scalar(raw_value)
        rows.append(row)
    return rows


def _mapping_lines(lines: list[str], key: str, *, indent: int) -> list[int]:
    marker = f"{key}:"
    return [
        index
        for index, line in enumerate(lines)
        if _indent(line) == indent and line.strip() == marker and not _comment(line)
    ]


def _flat_mapping(
    lines: list[str],
    start: int,
    end: int,
    indent: int,
    *,
    owner: str,
) -> dict[str, str]:
    result: dict[str, str] = {}
    for index in range(start, end):
        line = lines[index]
        if _indent(line) != indent or _comment(line) or ":" not in line:
            continue
        key, value = line.strip().split(":", 1)
        key = _scalar(key)
        if key == "<<":
            raise WorkflowContractError(f"{owner}: YAML merge keys are unsupported")
        if key in result:
            raise WorkflowContractError(f"{owner}: duplicate mapping key {key!r}")
        value = value.strip()
        _reject_unmodeled_scalar(value, f"{owner}:{key}")
        if _is_block_scalar(value) and owner.endswith(":permissions"):
            raise WorkflowContractError(f"{owner}:{key}: block scalar values are unsupported")
        if not value:
            raise WorkflowContractError(f"{owner}: nested mapping value for {key!r} is unsupported")
        if _is_block_scalar(value):
            block_end = _next_at_most_indent(lines, index + 1, end, indent)
            result[key] = _block_scalar_text(lines[index + 1 : block_end])
        else:
            result[key] = _scalar(value)
    return result


def _scalar_list(
    lines: list[str],
    start: int,
    end: int,
    indent: int,
    *,
    owner: str,
) -> list[str]:
    values: list[str] = []
    for line in lines[start:end]:
        if _comment(line) or not line.strip():
            continue
        if _indent(line) != indent or not line.lstrip().startswith("- "):
            raise WorkflowContractError(f"{owner}: expected a flat scalar list")
        value = line.lstrip()[2:].strip()
        _reject_unmodeled_scalar(value, owner)
        if not value:
            raise WorkflowContractError(f"{owner}: list values must not be empty")
        values.append(_scalar(value))
    if not values:
        raise WorkflowContractError(f"{owner}: list must not be empty")
    return values


def _next_at_most_indent(lines: list[str], start: int, end: int, indent: int) -> int:
    for index in range(start, end):
        line = lines[index]
        if line.strip() and _indent(line) <= indent:
            return index
    return end


def _indent(line: str) -> int:
    return len(line) - len(line.lstrip(" "))


def _comment(line: str) -> bool:
    return line.lstrip().startswith("#")


def _scalar(value: str) -> str:
    if len(value) >= 2 and value[0] == value[-1] and value[0] in {"'", '"'}:
        return value[1:-1]
    return value


def _reject_unmodeled_scalar(value: str, owner: str) -> None:
    """Reject YAML constructs this narrow structural reader cannot interpret safely."""
    if value.startswith(("&", "*", "!")):
        raise WorkflowContractError(f"{owner}: YAML anchors, aliases, and tags are unsupported")
    if value.startswith(("|", ">")) and not _is_block_scalar(value):
        raise WorkflowContractError(f"{owner}: unsupported block scalar header {value!r}")


def _is_block_scalar(value: str) -> bool:
    """Accept only the literal block scalar form used by repository workflows."""
    return value == "|"


def _block_scalar_text(lines: list[str]) -> str:
    indents = [_indent(line) for line in lines if line.strip()]
    content_indent = min(indents, default=0)
    return "\n".join(
        line[content_indent:] if line.strip() else "" for line in lines
    )

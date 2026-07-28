#!/usr/bin/env python3
"""Verify governed RustSec exceptions against deny.toml and exact repository inputs."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from datetime import date, timedelta
from pathlib import Path
import re
import sys
import tomllib
from typing import Any

try:
    from scripts import strict_json
except ModuleNotFoundError:
    import strict_json


REPO_ROOT = Path(__file__).resolve().parents[1]
LEDGER_PATH = Path("docs/security/RUSTSEC_EXCEPTIONS.json")
DENY_PATH = Path("deny.toml")
LOCK_PATH = Path("Cargo.lock")
ARTIFACT_PROFILES_PATH = Path("capabilities/artifact-profiles-v1.json")
SCHEMA_VERSION = 1
MAX_REVIEW_INTERVAL = timedelta(days=92)
ADVISORY_ID_RE = re.compile(r"^RUSTSEC-[0-9]{4}-[0-9]{4}$")
UPSTREAM_ISSUE_RE = re.compile(
    r"^https://github\.com/[^/]+/[^/]+/issues/[1-9][0-9]*$"
)


class RustSecExceptionError(RuntimeError):
    """The RustSec exception ledger is incomplete or inconsistent."""


STRICT_JSON = strict_json.StrictJsonContract(
    RustSecExceptionError,
    read_error_prefix="could not read",
)
load_json_strict = STRICT_JSON.load
require_object = STRICT_JSON.object
require_array = STRICT_JSON.array
require_exact_fields = STRICT_JSON.exact_fields
expect_string = STRICT_JSON.string


@dataclass(frozen=True)
class LockPackage:
    name: str
    version: str
    dependencies: tuple[str, ...]


@dataclass(frozen=True)
class RustSecException:
    advisory_id: str
    package_name: str
    package_version: str
    reason: str
    dependency_paths: tuple[tuple[str, ...], ...]
    affected_artifact_profiles: tuple[str, ...]
    upstream_issue: str
    owner: str
    reviewed_on: date
    review_due: date
    exit_condition: str


def load_exception_records(
    root: Path = REPO_ROOT,
    *,
    today: date | None = None,
) -> tuple[RustSecException, ...]:
    ledger = require_object(load_json_strict(root / LEDGER_PATH), "RustSec exception ledger")
    require_exact_fields(
        ledger,
        {"schema_version", "exceptions"},
        "RustSec exception ledger",
    )
    if ledger["schema_version"] != SCHEMA_VERSION:
        raise RustSecExceptionError(
            f"RustSec exception ledger schema_version must be {SCHEMA_VERSION}"
        )

    artifact_profiles = load_artifact_profile_ids(root)
    lock_packages = load_lock_packages(root)
    review_date = today or date.today()
    records = tuple(
        parse_exception(
            raw,
            index=index,
            artifact_profiles=artifact_profiles,
            lock_packages=lock_packages,
            today=review_date,
        )
        for index, raw in enumerate(
            require_array(ledger["exceptions"], "RustSec exception ledger exceptions")
        )
    )
    if not records:
        raise RustSecExceptionError("RustSec exception ledger must not be empty")
    advisory_ids = tuple(record.advisory_id for record in records)
    if advisory_ids != tuple(sorted(set(advisory_ids))):
        raise RustSecExceptionError(
            "RustSec exceptions must be unique and sorted by advisory id"
        )
    validate_deny_exceptions(root, records)
    return records


def parse_exception(
    raw: Any,
    *,
    index: int,
    artifact_profiles: frozenset[str],
    lock_packages: Mapping[tuple[str, str], tuple[LockPackage, ...]],
    today: date,
) -> RustSecException:
    context = f"RustSec exception[{index}]"
    value = require_object(raw, context)
    require_exact_fields(
        value,
        {
            "id",
            "package",
            "reason",
            "dependency_paths",
            "affected_artifact_profiles",
            "upstream_issue",
            "owner",
            "reviewed_on",
            "review_due",
            "exit_condition",
        },
        context,
    )
    advisory_id = expect_string(value["id"], f"{context}.id")
    if not ADVISORY_ID_RE.fullmatch(advisory_id):
        raise RustSecExceptionError(f"{context}.id must be a RustSec advisory id")

    package = require_object(value["package"], f"{context}.package")
    require_exact_fields(package, {"name", "version"}, f"{context}.package")
    package_name = expect_string(package["name"], f"{context}.package.name")
    package_version = expect_string(package["version"], f"{context}.package.version")
    require_lock_package(lock_packages, package_name, package_version, f"{context}.package")

    dependency_paths = tuple(
        parse_dependency_path(
            path,
            context=f"{context}.dependency_paths[{path_index}]",
            advisory_package=(package_name, package_version),
            lock_packages=lock_packages,
        )
        for path_index, path in enumerate(
            require_array(value["dependency_paths"], f"{context}.dependency_paths")
        )
    )
    if not dependency_paths:
        raise RustSecExceptionError(f"{context}.dependency_paths must not be empty")
    if dependency_paths != tuple(sorted(set(dependency_paths))):
        raise RustSecExceptionError(
            f"{context}.dependency_paths must be unique and sorted"
        )

    affected_profiles = expect_sorted_string_array(
        value["affected_artifact_profiles"],
        f"{context}.affected_artifact_profiles",
    )
    if not affected_profiles:
        raise RustSecExceptionError(
            f"{context}.affected_artifact_profiles must not be empty"
        )
    unknown_profiles = sorted(set(affected_profiles) - artifact_profiles)
    if unknown_profiles:
        raise RustSecExceptionError(
            f"{context}.affected_artifact_profiles references unknown profiles: "
            + ", ".join(unknown_profiles)
        )

    upstream_issue = expect_string(value["upstream_issue"], f"{context}.upstream_issue")
    if not UPSTREAM_ISSUE_RE.fullmatch(upstream_issue):
        raise RustSecExceptionError(
            f"{context}.upstream_issue must be an exact GitHub issue URL"
        )
    reviewed_on = parse_date(value["reviewed_on"], f"{context}.reviewed_on")
    review_due = parse_date(value["review_due"], f"{context}.review_due")
    if reviewed_on > today:
        raise RustSecExceptionError(f"{context}.reviewed_on must not be in the future")
    if review_due < today:
        raise RustSecExceptionError(
            f"{context} review expired on {review_due.isoformat()}"
        )
    interval = review_due - reviewed_on
    if interval <= timedelta(0) or interval > MAX_REVIEW_INTERVAL:
        raise RustSecExceptionError(
            f"{context} review interval must be between 1 and "
            f"{MAX_REVIEW_INTERVAL.days} days"
        )

    return RustSecException(
        advisory_id=advisory_id,
        package_name=package_name,
        package_version=package_version,
        reason=expect_string(value["reason"], f"{context}.reason"),
        dependency_paths=dependency_paths,
        affected_artifact_profiles=affected_profiles,
        upstream_issue=upstream_issue,
        owner=expect_string(value["owner"], f"{context}.owner"),
        reviewed_on=reviewed_on,
        review_due=review_due,
        exit_condition=expect_string(value["exit_condition"], f"{context}.exit_condition"),
    )


def validate_profile_coverage(
    records: Sequence[RustSecException],
    profile_packages: Mapping[str, frozenset[tuple[str, str]]],
) -> None:
    """Require each ledger profile list to equal target-scoped closure coverage."""
    if not profile_packages:
        raise RustSecExceptionError("artifact profile package observations must not be empty")
    failures: list[str] = []
    for record in records:
        observed = tuple(
            sorted(
                profile_id
                for profile_id, packages in profile_packages.items()
                if (record.package_name, record.package_version) in packages
            )
        )
        if observed != record.affected_artifact_profiles:
            failures.append(
                f"{record.advisory_id} affected_artifact_profiles drifted: "
                f"ledger={record.affected_artifact_profiles!r} observed={observed!r}"
            )
    if failures:
        raise RustSecExceptionError(
            "RustSec artifact-profile coverage verification failed:\n- "
            + "\n- ".join(failures)
        )


def validate_deny_exceptions(
    root: Path,
    records: Sequence[RustSecException],
) -> None:
    try:
        deny = tomllib.loads((root / DENY_PATH).read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise RustSecExceptionError(f"could not read {DENY_PATH}: {error}") from error
    advisories = require_object(deny.get("advisories"), "deny.toml advisories")
    ignores = require_array(advisories.get("ignore"), "deny.toml advisories.ignore")
    deny_entries: dict[str, str] = {}
    for index, raw in enumerate(ignores):
        context = f"deny.toml advisories.ignore[{index}]"
        value = require_object(raw, context)
        require_exact_fields(value, {"id", "reason"}, context)
        advisory_id = expect_string(value["id"], f"{context}.id")
        if advisory_id in deny_entries:
            raise RustSecExceptionError(f"deny.toml repeats advisory {advisory_id}")
        deny_entries[advisory_id] = expect_string(value["reason"], f"{context}.reason")
    ledger_entries = {record.advisory_id: record.reason for record in records}
    if deny_entries != ledger_entries:
        raise RustSecExceptionError(
            "deny.toml advisory ignores must exactly match the governed RustSec ledger"
        )


def load_artifact_profile_ids(root: Path) -> frozenset[str]:
    authority = require_object(
        load_json_strict(root / ARTIFACT_PROFILES_PATH),
        "artifact profile authority",
    )
    if authority.get("schema_version") != 1:
        raise RustSecExceptionError("artifact profile authority schema_version must be 1")
    profile_ids: list[str] = []
    for index, raw in enumerate(
        require_array(authority.get("profiles"), "artifact profile authority profiles")
    ):
        profile = require_object(raw, f"artifact profile[{index}]")
        profile_ids.append(expect_string(profile.get("id"), f"artifact profile[{index}].id"))
    if len(profile_ids) != len(set(profile_ids)):
        raise RustSecExceptionError("artifact profile authority repeats a profile id")
    return frozenset(profile_ids)


def load_lock_packages(
    root: Path,
) -> dict[tuple[str, str], tuple[LockPackage, ...]]:
    try:
        lock = tomllib.loads((root / LOCK_PATH).read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError) as error:
        raise RustSecExceptionError(f"could not read {LOCK_PATH}: {error}") from error
    packages: dict[tuple[str, str], list[LockPackage]] = {}
    for index, raw in enumerate(require_array(lock.get("package"), "Cargo.lock packages")):
        context = f"Cargo.lock package[{index}]"
        value = require_object(raw, context)
        name = expect_string(value.get("name"), f"{context}.name")
        version = expect_string(value.get("version"), f"{context}.version")
        dependencies = tuple(
            expect_string(dependency, f"{context}.dependencies[{dependency_index}]")
            for dependency_index, dependency in enumerate(value.get("dependencies", []))
        )
        packages.setdefault((name, version), []).append(
            LockPackage(name=name, version=version, dependencies=dependencies)
        )
    return {key: tuple(values) for key, values in packages.items()}


def parse_dependency_path(
    raw: Any,
    *,
    context: str,
    advisory_package: tuple[str, str],
    lock_packages: Mapping[tuple[str, str], tuple[LockPackage, ...]],
) -> tuple[str, ...]:
    path = tuple(
        expect_string(item, f"{context}[{index}]")
        for index, item in enumerate(require_array(raw, context))
    )
    if len(path) < 2:
        raise RustSecExceptionError(f"{context} must contain at least two packages")
    parsed = tuple(parse_package_ref(item, f"{context}[{index}]") for index, item in enumerate(path))
    if parsed[-1] != advisory_package:
        raise RustSecExceptionError(
            f"{context} must end at {advisory_package[0]}@{advisory_package[1]}"
        )
    for package_index, package in enumerate(parsed):
        require_lock_package(lock_packages, *package, f"{context}[{package_index}]")
    for edge_index, (parent, child) in enumerate(zip(parsed, parsed[1:])):
        candidates = lock_packages[parent]
        if not any(
            any(lock_dependency_matches(dependency, child) for dependency in candidate.dependencies)
            for candidate in candidates
        ):
            raise RustSecExceptionError(
                f"{context} has no Cargo.lock edge from "
                f"{parent[0]}@{parent[1]} to {child[0]}@{child[1]} at index {edge_index}"
            )
    return path


def parse_package_ref(value: str, context: str) -> tuple[str, str]:
    name, separator, version = value.rpartition("@")
    if not separator or not name or not version or name.strip() != name or version.strip() != version:
        raise RustSecExceptionError(f"{context} must use the exact name@version form")
    return name, version


def lock_dependency_matches(dependency: str, child: tuple[str, str]) -> bool:
    name, *suffix = dependency.split()
    if name != child[0]:
        return False
    if not suffix or suffix[0].startswith("("):
        return True
    return suffix[0] == child[1]


def require_lock_package(
    lock_packages: Mapping[tuple[str, str], tuple[LockPackage, ...]],
    name: str,
    version: str,
    context: str,
) -> None:
    if (name, version) not in lock_packages:
        raise RustSecExceptionError(
            f"{context} references missing Cargo.lock package {name}@{version}"
        )


def parse_date(value: Any, context: str) -> date:
    raw = expect_string(value, context)
    try:
        parsed = date.fromisoformat(raw)
    except ValueError as error:
        raise RustSecExceptionError(f"{context} must be an ISO 8601 date") from error
    if parsed.isoformat() != raw:
        raise RustSecExceptionError(f"{context} must be a canonical ISO 8601 date")
    return parsed


def expect_sorted_string_array(value: Any, context: str) -> tuple[str, ...]:
    items = tuple(
        expect_string(item, f"{context}[{index}]")
        for index, item in enumerate(require_array(value, context))
    )
    if items != tuple(sorted(set(items))):
        raise RustSecExceptionError(f"{context} must be unique and sorted")
    return items


def main() -> int:
    try:
        records = load_exception_records()
    except RustSecExceptionError as error:
        print(f"RustSec exception verification failed: {error}", file=sys.stderr)
        return 1
    print(f"RustSec exceptions: ok ({len(records)} governed exceptions)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

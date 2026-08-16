#!/usr/bin/env python3
"""Plan pull-request CI owners and aggregate their same-run results."""

from __future__ import annotations

import argparse
import json
import os
import posixpath
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


OWNER_NAMES = (
    "cli",
    "core",
    "fuzz",
    "hygiene",
    "node",
    "npm",
    "performance",
    "platform",
    "python",
    "security",
    "typst",
    "vscode",
    "web",
    "workflow",
)

_ALL_OWNERS = frozenset(OWNER_NAMES)
_CORE_OWNERS = frozenset({"core", "hygiene"})
_CRATE_OWNER_RULES = (
    ("crates/merman-android-jni/", {"core", "hygiene", "platform"}),
    (
        "crates/merman-ascii-test-contracts/",
        {"core", "hygiene", "node", "npm", "platform", "python", "web"},
    ),
    (
        "crates/merman-bindings-core/",
        {"core", "hygiene", "node", "npm", "platform", "python", "web"},
    ),
    ("crates/merman-cli/", {"cli", "core", "hygiene"}),
    ("crates/merman-export/", {"cli", "core", "hygiene"}),
    ("crates/merman-ffi/", {"core", "hygiene", "platform"}),
    ("crates/merman-node/", {"core", "hygiene", "node", "npm", "security"}),
    ("crates/merman-typst-plugin/", {"core", "hygiene", "typst"}),
    ("crates/merman-uniffi/", {"core", "hygiene", "platform", "python"}),
    ("crates/merman-wasm/", {"core", "hygiene", "npm", "web"}),
    (
        "crates/merman-analysis/",
        {"core", "hygiene", "npm", "vscode", "web"},
    ),
    (
        "crates/merman-editor-core/",
        {"core", "hygiene", "npm", "vscode", "web"},
    ),
    ("crates/merman-lsp/", {"core", "hygiene", "npm", "vscode", "web"}),
    ("crates/merman/", {"core", "hygiene"}),
    ("crates/dugong/", _CORE_OWNERS),
    ("crates/dugong-graphlib/", _CORE_OWNERS),
    ("crates/manatee/", _CORE_OWNERS),
    ("crates/merman-ascii/", _CORE_OWNERS),
    ("crates/merman-core/", {"core", "fuzz", "hygiene"}),
    ("crates/merman-elk-layered/", _CORE_OWNERS),
    ("crates/merman-fixture-render-context/", _CORE_OWNERS),
    ("crates/merman-layout-elk/", _CORE_OWNERS),
    ("crates/merman-render/", _CORE_OWNERS),
    ("crates/merman-rustdoc/", _CORE_OWNERS),
    ("crates/roughr/", _CORE_OWNERS),
    ("crates/xtask/src/cmd/typst_", {"core", "hygiene", "typst"}),
    (
        "crates/xtask/src/cmd/editor_",
        {"core", "hygiene", "npm", "vscode", "web"},
    ),
    (
        "crates/xtask/src/cmd/playground_",
        {"core", "hygiene", "npm", "vscode", "web"},
    ),
    ("crates/xtask/src/cmd/native_abi.rs", {"core", "hygiene", "platform"}),
    ("crates/xtask/", _CORE_OWNERS),
)
_SCRIPT_EXACT_OWNER_RULES = {
    "scripts/audit_plan.py": frozenset({"hygiene", "npm", "security"}),
    "scripts/strict_json.py": frozenset({"hygiene"}),
    "scripts/test_audit_plan.py": frozenset({"hygiene", "npm", "security"}),
    "scripts/test_build_android.py": frozenset({"hygiene", "platform"}),
    "scripts/test_publish.py": frozenset({"hygiene"}),
}
_SCRIPT_PREFIX_OWNER_RULES = (
    ("scripts/build-python-", {"hygiene", "python"}),
    ("scripts/python_", {"hygiene", "python"}),
    ("scripts/test_python_", {"hygiene", "python"}),
    ("scripts/build-apple-", {"hygiene", "platform"}),
    ("scripts/flutter_", {"hygiene", "platform"}),
    ("scripts/native_symbol_", {"hygiene", "platform"}),
    ("scripts/test_flutter_", {"hygiene", "platform"}),
    ("scripts/test_native_symbol_", {"hygiene", "platform"}),
    ("scripts/test_verify_platform_", {"hygiene", "platform"}),
    ("scripts/test_verify_flutter_", {"hygiene", "platform"}),
    ("scripts/node_", {"hygiene", "node", "npm"}),
    ("scripts/test_node_", {"hygiene", "node", "npm"}),
    ("scripts/npm_", {"hygiene", "npm"}),
    ("scripts/test_web_", {"hygiene", "npm", "web"}),
    ("scripts/web_", {"hygiene", "npm", "web"}),
    ("scripts/check-svg-", {"hygiene", "npm", "vscode", "web"}),
    ("scripts/generate-svg-", {"hygiene", "npm", "vscode", "web"}),
    ("scripts/svg-", {"hygiene", "npm", "vscode", "web"}),
    ("scripts/cli_", {"cli", "hygiene"}),
    ("scripts/generate_cli_", {"cli", "hygiene"}),
    ("scripts/test_cli_", {"cli", "hygiene"}),
    ("scripts/test_generate_cli_", {"cli", "hygiene"}),
    ("scripts/test_nix_", {"cli", "hygiene"}),
    ("scripts/test_verify_cli_", {"cli", "hygiene"}),
    ("scripts/test_verify_homebrew_", {"cli", "hygiene"}),
    ("scripts/verify_rustsec_", {"hygiene", "security"}),
    ("scripts/test_verify_rustsec_", {"hygiene", "security"}),
)
_HYGIENE_SCRIPT_PREFIXES = (
    "scripts/adr_",
    "scripts/artifact_",
    "scripts/capability_",
    "scripts/crates_io_",
    "scripts/generate-npm-license-",
    "scripts/generate-rust-license-",
    "scripts/release-",
    "scripts/release_",
    "scripts/sync-release-",
    "scripts/test_adr_",
    "scripts/test_artifact_",
    "scripts/test_crates_io_",
    "scripts/test_generate_rust_license_",
    "scripts/test_release_",
    "scripts/test_sync_",
    "scripts/test_verify_artifact_",
    "scripts/test_verify_crate_",
    "scripts/test_verify_independent_",
    "scripts/test_verify_third_",
    "scripts/verify-independent-",
    "scripts/verify_artifact_",
    "scripts/verify_crate_",
    "scripts/verify-third-",
)
_NONCANONICAL_SCRIPT_SUFFIXES = (
    ".backup",
    ".bak",
    ".generated",
    ".orig",
    ".tmp",
    "~",
)
_STATUS_PATH_COUNTS = {
    "A": 1,
    "B": 1,
    "D": 1,
    "M": 1,
    "T": 1,
    "U": 1,
    "X": 1,
    "C": 2,
    "R": 2,
}


class GateError(ValueError):
    """The aggregate cannot prove that every selected owner succeeded."""


@dataclass(frozen=True)
class Change:
    status: str
    paths: tuple[str, ...]


def parse_name_status_z(raw: bytes) -> list[Change]:
    """Parse ``git diff --name-status -z`` without line-oriented assumptions."""

    if not raw:
        return []
    if not raw.endswith(b"\0"):
        raise ValueError("name-status data is not NUL terminated")

    fields = raw.split(b"\0")
    fields.pop()
    changes: list[Change] = []
    index = 0
    while index < len(fields):
        status = fields[index].decode("ascii", "strict")
        index += 1
        kind = status[:1]
        path_count = _STATUS_PATH_COUNTS.get(kind)
        if path_count is None or (kind in {"C", "R"} and not status[1:].isdigit()):
            raise ValueError(f"unsupported git name-status token: {status!r}")
        if index + path_count > len(fields):
            raise ValueError(f"incomplete git name-status record: {status!r}")

        paths = tuple(
            _decode_and_validate_path(field) for field in fields[index : index + path_count]
        )
        index += path_count
        changes.append(Change(status=status, paths=paths))
    return changes


def plan_changes(changes: Sequence[Change], *, base: str, head: str) -> dict[str, Any]:
    """Return a deterministic owner plan for parsed repository changes."""

    owners = {name: False for name in OWNER_NAMES}
    reasons: dict[str, list[str]] = {name: [] for name in OWNER_NAMES}
    if not changes:
        return _plan_document(
            base=base,
            head=head,
            changes=[],
            owners=owners,
            reasons=reasons,
            fallback=False,
            fallback_reason=None,
            empty=True,
        )

    fallback_reason: str | None = None
    for change in changes:
        for path in change.paths:
            selected, reason, broad = _classify_path(path)
            if broad:
                fallback_reason = reason
                break
            for owner in selected:
                owners[owner] = True
                reasons[owner].append(reason)
        if fallback_reason is not None:
            break

    if fallback_reason is not None:
        owners = {name: True for name in OWNER_NAMES}
        reasons = {name: [fallback_reason] for name in OWNER_NAMES}

    return _plan_document(
        base=base,
        head=head,
        changes=[{"status": change.status, "paths": list(change.paths)} for change in changes],
        owners=owners,
        reasons=reasons,
        fallback=fallback_reason is not None,
        fallback_reason=fallback_reason,
        empty=False,
    )


def plan_repository_diff(repository: Path, base: str, head: str) -> dict[str, Any]:
    """Resolve an exact Git diff, selecting every owner when Git cannot prove it."""

    command = [
        "git",
        "-C",
        os.fspath(repository),
        "diff",
        "--name-status",
        "-z",
        "--find-renames",
        "--no-ext-diff",
        base,
        head,
        "--",
    ]
    try:
        completed = subprocess.run(command, check=False, capture_output=True)
    except OSError as exc:
        return _fallback_plan(base, head, f"git diff failed to start: {exc}")
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", "replace").strip()
        detail = f": {diagnostic}" if diagnostic else ""
        return _fallback_plan(base, head, f"git diff failed with exit {completed.returncode}{detail}")
    try:
        changes = parse_name_status_z(completed.stdout)
    except ValueError as exc:
        return _fallback_plan(base, head, f"git diff output was malformed: {exc}")
    return plan_changes(changes, base=base, head=head)


def plan_all(*, base: str, head: str, reason: str) -> dict[str, Any]:
    """Select every owner for an explicit full lifecycle such as a main push."""

    if not reason.strip():
        raise ValueError("full-plan reason must not be empty")
    return _plan_document(
        base=base,
        head=head,
        changes=[],
        owners={name: True for name in OWNER_NAMES},
        reasons={name: [reason] for name in OWNER_NAMES},
        fallback=False,
        fallback_reason=None,
        empty=False,
    )


def plan_selected(
    *, base: str, head: str, selected: Iterable[str], reason: str
) -> dict[str, Any]:
    """Select explicit owners for a non-PR lifecycle such as a host safety-net run."""

    selected_owners = frozenset(selected)
    unknown = selected_owners - _ALL_OWNERS
    if unknown:
        raise ValueError(f"unknown explicit CI owners: {sorted(unknown)}")
    if not selected_owners:
        raise ValueError("explicit CI owner selection must not be empty")
    if not reason.strip():
        raise ValueError("explicit CI owner reason must not be empty")
    return _plan_document(
        base=base,
        head=head,
        changes=[],
        owners={name: name in selected_owners for name in OWNER_NAMES},
        reasons={name: [reason] if name in selected_owners else [] for name in OWNER_NAMES},
        fallback=False,
        fallback_reason=None,
        empty=False,
    )


def evaluate_gate(plan: Mapping[str, Any], jobs: Mapping[str, Any]) -> dict[str, Any]:
    """Fail closed unless every selected owner has a successful required job."""

    normalized_plan = _validate_plan(plan)
    if not isinstance(jobs, Mapping):
        raise GateError("job results must be a JSON object")

    owner_jobs: dict[str, list[tuple[str, bool, str]]] = {name: [] for name in OWNER_NAMES}
    failures: list[str] = []
    skipped: list[str] = []
    selected_jobs: list[str] = []
    allowed_results = {"", "cancelled", "failure", "skipped", "success"}

    for job_name, raw_entry in sorted(jobs.items()):
        if not isinstance(job_name, str) or not job_name:
            raise GateError("job result names must be non-empty strings")
        if not isinstance(raw_entry, Mapping):
            raise GateError(f"job {job_name!r} must be a JSON object")
        if set(raw_entry) != {"owner", "required", "result"}:
            raise GateError(f"job {job_name!r} has an invalid result shape")
        owner = raw_entry["owner"]
        required = raw_entry["required"]
        result = raw_entry["result"]
        if owner not in _ALL_OWNERS:
            raise GateError(f"job {job_name!r} names unknown owner {owner!r}")
        if not isinstance(required, bool):
            raise GateError(f"job {job_name!r} required flag must be boolean")
        if not isinstance(result, str) or result not in allowed_results:
            raise GateError(f"job {job_name!r} has invalid result {result!r}")
        owner_jobs[owner].append((job_name, required, result))

        if result in {"cancelled", "failure"}:
            failures.append(f"{job_name} ended with {result}")

    for owner, selected in normalized_plan["owners"].items():
        entries = owner_jobs[owner]
        if not selected:
            skipped.extend(
                job_name for job_name, _required, result in entries if result in {"", "skipped"}
            )
            continue

        required_entries = [entry for entry in entries if entry[1]]
        if not required_entries:
            failures.append(f"selected owner {owner} has no required same-run job")
            continue
        for job_name, _required, result in required_entries:
            selected_jobs.append(job_name)
            if result != "success":
                failures.append(
                    f"selected owner {owner} job {job_name} ended with {result or 'missing'}"
                )

    if failures:
        raise GateError("; ".join(dict.fromkeys(failures)))

    return {
        "schema_version": 1,
        "selected": sorted(selected_jobs),
        "skipped": sorted(skipped),
    }


def _decode_and_validate_path(raw: bytes) -> str:
    path = raw.decode("utf-8", "surrogateescape")
    if not path or "\0" in path or path.startswith("/") or path == ".":
        raise ValueError(f"unsafe repository path: {path!r}")
    if ".." in path.split("/"):
        raise ValueError(f"unsafe repository path: {path!r}")
    if posixpath.normpath(path) != path:
        raise ValueError(f"non-canonical repository path: {path!r}")
    return path


def _classify_path(path: str) -> tuple[frozenset[str], str, bool]:
    workflow_paths = (
        ".github/workflows/",
        ".github/actions/",
    )
    if path.startswith(workflow_paths) or path in {
        ".github/actionlint.yaml",
        ".github/dependabot.yml",
        ".github/zizmor.yml",
        "scripts/ci_plan.py",
        "scripts/test_ci_plan.py",
    }:
        return _ALL_OWNERS, f"workflow or classifier changed: {path}", True

    if path in {"Cargo.lock", "Cargo.toml", "rust-toolchain.toml", "dist-workspace.toml"}:
        return _ALL_OWNERS, f"shared Rust authority changed: {path}", True
    if path.startswith("capabilities/"):
        return _ALL_OWNERS, f"shared capability schema changed: {path}", True
    if path.startswith("crates/"):
        for prefix, selected in _CRATE_OWNER_RULES:
            if path.startswith(prefix):
                owners = set(selected)
                if "/benches/" in path:
                    owners.add("performance")
                return frozenset(owners), f"Rust crate owner changed: {path}", False
        return _ALL_OWNERS, f"unclassified Rust crate changed: {path}", True
    if path.startswith("fixtures/bindings/"):
        return (
            frozenset({"core", "hygiene", "node", "npm", "platform", "python", "web"}),
            f"binding fixture owner changed: {path}",
            False,
        )
    if path.startswith("fixtures/"):
        return _CORE_OWNERS, f"renderer fixture owner changed: {path}", False
    if path.startswith("tools/upstreams/"):
        return (
            frozenset({"core", "hygiene", "npm", "web"}),
            f"upstream runtime authority changed: {path}",
            False,
        )
    if path.startswith("scripts/"):
        if path.endswith(_NONCANONICAL_SCRIPT_SUFFIXES):
            return _ALL_OWNERS, f"noncanonical repository script changed: {path}", True
        if selected := _SCRIPT_EXACT_OWNER_RULES.get(path):
            return selected, f"owner script changed: {path}", False
        for prefix, selected in _SCRIPT_PREFIX_OWNER_RULES:
            if path.startswith(prefix):
                return frozenset(selected), f"owner script changed: {path}", False
        if path.startswith(_HYGIENE_SCRIPT_PREFIXES):
            return frozenset({"hygiene"}), f"repository hygiene script changed: {path}", False
        return _ALL_OWNERS, f"unclassified repository script changed: {path}", True

    if path.startswith(
        "playground/editor-artifact-receipt-v"
    ) and path.endswith(".json"):
        return (
            frozenset({"hygiene", "web"}),
            f"Web editor artifact receipt changed: {path}",
            False,
        )
    if path.startswith("docs/"):
        owners = {"hygiene"}
        if path.startswith("docs/performance/"):
            owners.add("performance")
        elif path.startswith("docs/security/"):
            owners.add("security")
        elif path == "docs/release/WASM_SIZE_BUDGETS.json":
            owners.update({"typst", "web"})
        return frozenset(owners), f"documentation owner changed: {path}", False

    if path in {
        "platforms/web/src/svg-safety-policy.ts",
        "tools/vscode-extension/src/preview-svg-safety-policy.ts",
    }:
        return (
            frozenset({"hygiene", "npm", "vscode", "web"}),
            f"shared SVG safety policy changed: {path}",
            False,
        )
    if path == "playground/examples/manifest.json":
        return (
            frozenset({"hygiene", "npm", "vscode", "web"}),
            f"shared editor example manifest changed: {path}",
            False,
        )
    if path.startswith(("platforms/web/", "playground/")):
        return frozenset({"hygiene", "npm", "web"}), f"web owner changed: {path}", False
    if path.startswith(("platforms/node/", "packages/node")):
        return (
            frozenset({"core", "hygiene", "node", "npm", "security"}),
            f"Node owner changed: {path}",
            False,
        )
    if path.startswith("contracts/editor-language/"):
        return (
            frozenset({"hygiene", "npm", "vscode", "web"}),
            f"shared editor language authority changed: {path}",
            False,
        )
    if path.startswith("tools/vscode-extension/"):
        return frozenset({"hygiene", "npm", "vscode"}), f"VS Code owner changed: {path}", False
    if path.startswith("tools/bench/"):
        return (
            frozenset({"core", "hygiene", "performance"}),
            f"performance owner changed: {path}",
            False,
        )
    if path.startswith("platforms/python/"):
        owners = {"hygiene", "python"}
        if path.endswith(("Cargo.lock", "Cargo.toml")) or "/legal/" in path:
            owners.add("security")
        return frozenset(owners), f"Python owner changed: {path}", False
    if path.startswith(("platforms/android/", "platforms/apple/", "platforms/flutter/", "platforms/ios/")):
        return frozenset({"hygiene", "platform"}), f"native platform owner changed: {path}", False
    if path.startswith("fuzz/"):
        return (
            frozenset({"core", "fuzz", "hygiene", "security"}),
            f"fuzz owner changed: {path}",
            False,
        )
    if path.startswith(("nix/", "distribution/cli/")) or path in {
        "default.nix",
        "flake.lock",
        "flake.nix",
    }:
        return frozenset({"cli", "hygiene"}), f"CLI package owner changed: {path}", False
    if path.startswith(("contracts/abi/", "distribution/typst/")) or path == "Package.swift":
        return _ALL_OWNERS, f"shared package surface changed: {path}", True
    if path.startswith(("tools/mermaid-cli/", "tools/debug/", "tools/preview/")):
        return _ALL_OWNERS, f"shared tool changed: {path}", True

    if path in {"README.md", "CHANGELOG.md", "CONTEXT.md", "AGENTS.md", ".agents"}:
        return frozenset({"hygiene"}), f"repository documentation changed: {path}", False
    if path.startswith(".agents/"):
        return frozenset({"hygiene"}), f"agent documentation changed: {path}", False
    if path.startswith("assets/"):
        return frozenset({"hygiene", "web"}), f"user-facing asset changed: {path}", False
    if path.startswith("THIRD_PARTY_LICENSES/") or path in {
        "LICENSE-APACHE",
        "LICENSE-MIT",
        "THIRD_PARTY_NOTICES.md",
        "about.toml",
        "deny.toml",
    }:
        return _ALL_OWNERS, f"shared legal or dependency policy changed: {path}", True

    return _ALL_OWNERS, f"unclassified path changed: {path}", True


def _fallback_plan(base: str, head: str, reason: str) -> dict[str, Any]:
    return _plan_document(
        base=base,
        head=head,
        changes=[],
        owners={name: True for name in OWNER_NAMES},
        reasons={name: [reason] for name in OWNER_NAMES},
        fallback=True,
        fallback_reason=reason,
        empty=False,
    )


def _plan_document(
    *,
    base: str,
    head: str,
    changes: list[dict[str, Any]],
    owners: dict[str, bool],
    reasons: dict[str, list[str]],
    fallback: bool,
    fallback_reason: str | None,
    empty: bool,
) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "base": base,
        "head": head,
        "empty": empty,
        "fallback": fallback,
        "fallback_reason": fallback_reason,
        "changes": changes,
        "owners": owners,
        "reasons": {name: sorted(set(reasons[name])) for name in OWNER_NAMES},
    }


def _validate_plan(plan: Mapping[str, Any]) -> dict[str, Any]:
    if not isinstance(plan, Mapping):
        raise GateError("planner output must be a JSON object")
    required_keys = {
        "base",
        "changes",
        "empty",
        "fallback",
        "fallback_reason",
        "head",
        "owners",
        "reasons",
        "schema_version",
    }
    if (
        set(plan) != required_keys
        or type(plan.get("schema_version")) is not int
        or plan["schema_version"] != 1
    ):
        raise GateError("planner output has an invalid schema")
    for field in ("base", "head"):
        revision = plan[field]
        if not isinstance(revision, str) or re.fullmatch(r"[0-9a-f]{40}", revision) is None:
            raise GateError(f"planner {field} must be a full Git object ID")

    changes = plan["changes"]
    if not isinstance(changes, list):
        raise GateError("planner changes must be a JSON array")
    for change in changes:
        if not isinstance(change, Mapping) or set(change) != {"paths", "status"}:
            raise GateError("planner change has an invalid shape")
        status = change["status"]
        paths = change["paths"]
        if not isinstance(status, str) or not status:
            raise GateError("planner change status must be a non-empty string")
        kind = status[:1]
        expected_path_count = _STATUS_PATH_COUNTS.get(kind)
        if (
            expected_path_count is None
            or (
                kind in {"C", "R"}
                and (
                    not status[1:].isascii()
                    or not status[1:].isdigit()
                    or int(status[1:]) > 100
                )
            )
            or (kind not in {"C", "R"} and status != kind)
        ):
            raise GateError(f"planner change status is invalid: {status!r}")
        if not isinstance(paths, list) or len(paths) != expected_path_count:
            raise GateError("planner change paths must be a non-empty array")
        try:
            for path in paths:
                if not isinstance(path, str):
                    raise ValueError("path is not a string")
                _decode_and_validate_path(path.encode("utf-8", "surrogateescape"))
        except (UnicodeEncodeError, ValueError) as exc:
            raise GateError(f"planner change path is invalid: {exc}") from exc

    owners = plan.get("owners")
    reasons = plan.get("reasons")
    if not isinstance(owners, Mapping) or set(owners) != _ALL_OWNERS:
        raise GateError("planner output has an incomplete owner map")
    if not isinstance(reasons, Mapping) or set(reasons) != _ALL_OWNERS:
        raise GateError("planner output has an incomplete reason map")
    for owner in OWNER_NAMES:
        if not isinstance(owners[owner], bool):
            raise GateError(f"planner owner {owner} is not boolean")
        if not isinstance(reasons[owner], list) or not all(
            isinstance(reason, str) and reason.strip() for reason in reasons[owner]
        ):
            raise GateError(f"planner owner {owner} has invalid reasons")
        if owners[owner] and not reasons[owner]:
            raise GateError(f"selected planner owner {owner} has no reason")
    if not isinstance(plan["empty"], bool) or not isinstance(plan["fallback"], bool):
        raise GateError("planner flags must be boolean")
    fallback_reason = plan["fallback_reason"]
    if plan["fallback"]:
        if not isinstance(fallback_reason, str) or not fallback_reason.strip():
            raise GateError("fallback planner output must include a reason")
        if not all(owners.values()):
            raise GateError("fallback planner output must select every owner")
        if any(fallback_reason not in reasons[owner] for owner in OWNER_NAMES):
            raise GateError("fallback planner reason must explain every selected owner")
    elif fallback_reason is not None:
        raise GateError("non-fallback planner output cannot include a fallback reason")
    if plan["empty"] and any(owners.values()):
        raise GateError("an empty plan cannot select owner jobs")
    if plan["empty"] and changes:
        raise GateError("an empty plan cannot include changes")
    if plan["empty"] and plan["fallback"]:
        raise GateError("an empty plan cannot be a fallback")
    if not plan["empty"] and not any(owners.values()):
        raise GateError("a non-empty plan must select at least one owner")
    if changes:
        expected = plan_changes(
            [
                Change(status=change["status"], paths=tuple(change["paths"]))
                for change in changes
            ],
            base=plan["base"],
            head=plan["head"],
        )
        for field in (
            "changes",
            "empty",
            "fallback",
            "fallback_reason",
            "owners",
            "reasons",
        ):
            if plan[field] != expected[field]:
                raise GateError(f"planner {field} does not match classified changes")
    return dict(plan)


def _compact_json(document: Any) -> str:
    return json.dumps(document, ensure_ascii=True, separators=(",", ":"), sort_keys=True)


def _write_github_outputs(path: Path, plan: Mapping[str, Any]) -> None:
    lines = [f"plan={_compact_json(plan)}"]
    lines.extend(f"{owner}={str(plan['owners'][owner]).lower()}" for owner in OWNER_NAMES)
    with path.open("a", encoding="utf-8", newline="\n") as output:
        output.write("\n".join(lines) + "\n")


def _parse_json_argument(value: str, *, label: str) -> Any:
    try:
        return json.loads(value)
    except json.JSONDecodeError as exc:
        raise GateError(f"{label} is not valid JSON: {exc}") from exc


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    plan_parser = subparsers.add_parser("plan", help="select CI owners")
    plan_parser.add_argument("--base", required=True)
    plan_parser.add_argument("--head", required=True)
    plan_parser.add_argument("--repository", type=Path, default=Path.cwd())
    plan_parser.add_argument("--select-all", action="store_true")
    plan_parser.add_argument(
        "--select-owner",
        action="append",
        choices=OWNER_NAMES,
        default=[],
        help="select one owner for a non-PR lifecycle; may be repeated",
    )
    plan_parser.add_argument("--reason", default="explicit full lifecycle")
    plan_parser.add_argument("--github-output", type=Path)

    gate_parser = subparsers.add_parser("gate", help="aggregate same-run owner results")
    gate_parser.add_argument("--plan-json", required=True)
    gate_parser.add_argument("--jobs-json", required=True)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = _build_parser().parse_args(list(argv) if argv is not None else None)
    if args.command == "plan":
        if args.select_all and args.select_owner:
            raise SystemExit("--select-all and --select-owner are mutually exclusive")
        if args.select_all:
            plan = plan_all(base=args.base, head=args.head, reason=args.reason)
        elif args.select_owner:
            plan = plan_selected(
                base=args.base,
                head=args.head,
                selected=args.select_owner,
                reason=args.reason,
            )
        else:
            plan = plan_repository_diff(args.repository, args.base, args.head)
        encoded = _compact_json(plan)
        print(encoded)
        if args.github_output is not None:
            _write_github_outputs(args.github_output, plan)
        return 0

    try:
        plan = _parse_json_argument(args.plan_json, label="planner output")
        jobs = _parse_json_argument(args.jobs_json, label="job results")
        summary = evaluate_gate(plan, jobs)
    except GateError as exc:
        print(f"pr-gate failed closed: {exc}", file=sys.stderr)
        return 1
    print(_compact_json(summary))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

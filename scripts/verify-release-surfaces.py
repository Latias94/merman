#!/usr/bin/env python3
"""Verify release surface metadata against repository facts."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shlex
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SURFACES_PATH = ROOT / "docs" / "release" / "SURFACES.json"
REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS = {
    "playground/package.json",
    "playground/tests/package.json",
    "tools/mermaid-cli/package.json",
}
OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS = {
    "package.json",
    # U14 rejected these private comparison packages. They remain tracked only as evidence and
    # must never become an undeclared npm release surface.
    "platforms/node/package.json",
    "platforms/node/packages/node/package.json",
    "platforms/node/packages/node-darwin-arm64/package.json",
    "platforms/node/packages/node-darwin-x64/package.json",
    "platforms/node/packages/node-linux-x64-gnu/package.json",
    "platforms/node/packages/node-linux-x64-musl/package.json",
    "platforms/node/packages/node-win32-x64-msvc/package.json",
}
NON_SURFACE_PACKAGE_MANIFESTS = REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS | OPTIONAL_NON_SURFACE_PACKAGE_MANIFESTS
PACKAGE_INVENTORY_IGNORED_DIRECTORY_NAMES = {
    ".build",
    ".git",
    ".github",
    ".gradle",
    ".pytest_cache",
    ".runtime",
    ".vscode-test",
    "__pycache__",
    "coverage",
    "dist",
    "node_modules",
    "playwright-report",
    "repo-ref",
    "target",
    "test-results",
}
PACKAGE_INVENTORY_IGNORED_RELATIVE_DIRECTORIES = {
    "platforms/web/pkg",
}
GENERATED_SURFACES_BEGIN = "<!-- BEGIN GENERATED RELEASE SURFACES -->"
GENERATED_SURFACES_END = "<!-- END GENERATED RELEASE SURFACES -->"
WEB_SURFACE_DESCRIPTOR_PATH = "platforms/web/web-surface-descriptor.json"
ARTIFACT_PROFILES_PATH = "capabilities/artifact-profiles-v1.json"
OPERATIONAL_CHANNEL_STATES = {"published", "artifact-only"}
PROTECTED_PUBLICATION_KINDS = {
    "crates.io",
    "github-release-assets",
    "npm",
    "pypi",
    "pub.dev",
}


class CheckFailure(Exception):
    pass


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract", type=Path, default=SURFACES_PATH)
    parser.add_argument(
        "--check-ci-self",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Require CI to run this verifier and its unit tests.",
    )
    parser.add_argument(
        "--write-docs",
        action="store_true",
        help="Refresh the generated public surface table from SURFACES.json before checking.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    root = ROOT
    failures: list[str] = []

    try:
        contract = load_contract(args.contract)
    except CheckFailure as error:
        print(f"::error file={rel(args.contract, root)}::{error}", file=sys.stderr)
        return 1

    if args.write_docs:
        write_generated_surface_docs(root, contract)

    checks = [
        ("surface contract paths", lambda: check_surface_paths(root, contract)),
        ("workflow operation contract", lambda: check_workflow_operations(root, contract)),
        ("cargo-dist asset contract", lambda: check_cargo_dist_asset_contract(root, contract)),
        ("package manifest names", lambda: check_package_manifest_names(root, contract)),
        ("package manifest publishability", lambda: check_package_publishability(root, contract)),
        ("package manifest inventory", lambda: check_package_inventory(root, contract)),
        ("publishable crate inventory", lambda: check_publishable_crate_inventory(root, contract)),
        ("web package contract", lambda: check_web_contract(root, contract)),
        ("blocked channel metadata", lambda: check_blocked_channel_metadata(contract)),
    ]
    if args.check_ci_self:
        checks.append(("CI wiring", lambda: check_ci_wiring(root)))

    for label, check in checks:
        try:
            check()
            print(f"{label}: ok")
        except CheckFailure as error:
            failures.append(str(error))

    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    return 0


def load_contract(path: Path) -> dict[str, Any]:
    if not path.exists():
        raise CheckFailure(f"missing surface contract: {path}")

    try:
        data = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_json_keys,
        )
    except (OSError, json.JSONDecodeError) as error:
        raise CheckFailure(f"invalid surface contract {path}: {error}") from error
    release_status = load_release_status_module()
    try:
        release_status.validate_contract(data)
    except release_status.SurfaceError as error:
        raise CheckFailure(str(error)) from error
    return data


def reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise json.JSONDecodeError(f"duplicate object key {key!r}", "", 0)
        result[key] = value
    return result


def load_release_status_module() -> Any:
    module_path = ROOT / "scripts" / "release-status.py"
    spec = importlib.util.spec_from_file_location("release_status", module_path)
    if spec is None or spec.loader is None:
        raise CheckFailure("could not load scripts/release-status.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_web_package_group_module() -> Any:
    module_path = ROOT / "scripts" / "web_package_group.py"
    spec = importlib.util.spec_from_file_location("web_package_group", module_path)
    if spec is None or spec.loader is None:
        raise CheckFailure("could not load scripts/web_package_group.py")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def check_surface_paths(root: Path, contract: dict[str, Any]) -> None:
    require_file(root, "docs/release/SURFACES.json")

    for surface in contract["surfaces"]:
        for package in surface.get("packages", []):
            require_file(root, package["manifest"])
        for channel in surface.get("channels", []):
            workflow = channel.get("workflow")
            if channel["declared_state"] in OPERATIONAL_CHANNEL_STATES and not workflow:
                fail(
                    "docs/release/SURFACES.json",
                    f"{surface['id']}/{channel['id']}: operational channel must declare a workflow",
                )
            if workflow:
                require_file(root, workflow)
                if not workflow.startswith(".github/workflows/") or not workflow.endswith((".yml", ".yaml")):
                    fail(
                        "docs/release/SURFACES.json",
                        f"{surface['id']}/{channel['id']}: workflow must be a GitHub workflow YAML file",
                    )
            if channel["declared_state"] in OPERATIONAL_CHANNEL_STATES and not channel.get("workflow_job"):
                fail(
                    "docs/release/SURFACES.json",
                    f"{surface['id']}/{channel['id']}: operational channel must declare workflow_job",
                )


def check_workflow_operations(root: Path, contract: dict[str, Any]) -> None:
    workflow_contract = load_workflow_contract_module()
    documents: dict[str, dict[str, Any]] = {}

    for surface in contract["surfaces"]:
        for channel in surface.get("channels", []):
            if channel["declared_state"] not in OPERATIONAL_CHANNEL_STATES:
                continue
            owner = f"{surface['id']}/{channel['id']}"
            workflow = channel["workflow"]
            job_id = channel["workflow_job"]
            try:
                if workflow not in documents:
                    documents[workflow] = workflow_contract.load_workflow_contract(
                        require_file(root, workflow)
                    )
                job = workflow_contract.workflow_job(documents[workflow], job_id)
            except (OSError, workflow_contract.WorkflowContractError) as error:
                fail(workflow, f"{owner}: invalid workflow contract: {error}")

            if not workflow_job_performs_channel_operation(job, channel["kind"]):
                fail(
                    workflow,
                    f"{owner}: job {job_id!r} does not perform the declared {channel['kind']} operation",
                )
            check_workflow_operation_authority(
                documents[workflow],
                job,
                channel,
                workflow,
                owner,
            )
            if channel["kind"] == "github-actions-artifact":
                check_actions_artifact_contract(job, surface, channel, workflow, owner)


def load_workflow_contract_module() -> Any:
    module_path = ROOT / "scripts" / "github_workflow_contract.py"
    spec = importlib.util.spec_from_file_location("github_workflow_contract", module_path)
    if spec is None or spec.loader is None:
        raise CheckFailure("could not load scripts/github_workflow_contract.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def workflow_job_performs_channel_operation(job: dict[str, Any], kind: str) -> bool:
    if condition_is_always_false(job.get("if")):
        return False
    command_rules: dict[str, tuple[tuple[str, ...], ...]] = {
        "crates.io": (("cargo", "publish"),),
        "github-release-assets": (("gh", "release", "create"), ("gh", "release", "upload")),
        "homebrew": (("brew", "install"),),
        "npm": (("npm", "publish"),),
        "pub.dev": (("dart", "pub", "publish"),),
    }
    action_rules = {
        "github-actions-artifact": "actions/upload-artifact",
        "pypi": "pypa/gh-action-pypi-publish",
    }
    steps = job.get("steps", [])

    if kind in action_rules:
        expected = action_rules[kind]
        return any(
            not condition_is_always_false(step.get("if"))
            and
            isinstance(step.get("uses"), str)
            and step["uses"].partition("@")[0] == expected
            for step in steps
        )
    if kind in command_rules:
        return any(
            not condition_is_always_false(step.get("if"))
            and isinstance(step.get("run"), str)
            and operation_run_matches_kind(step["run"], kind, command_rules[kind])
            for step in steps
        )
    raise CheckFailure(f"no workflow operation rule for operational channel kind {kind!r}")


def operation_run_matches_kind(
    run: str,
    kind: str,
    command_rules: tuple[tuple[str, ...], ...],
) -> bool:
    if any(shell_run_invokes(run, command) for command in command_rules):
        return True
    return kind == "npm" and shell_run_invokes_web_package_group_reconcile(run)


def shell_run_invokes_web_package_group_reconcile(run: str) -> bool:
    logical_run = run.replace("\\\n", " ")
    for line in executable_shell_lines(logical_run):
        tokens = shell_command_tokens(line)
        if len(tokens) < 3 or tokens[0] != "python3":
            continue
        if tokens[1] != "scripts/web_package_group.py" or tokens[2] != "reconcile":
            continue
        if "--manifest" in tokens and "--artifact-dir" in tokens and "--report" in tokens:
            return True
    return False


def condition_is_always_false(value: Any) -> bool:
    if not isinstance(value, str):
        return False
    normalized = value.strip().lower()
    if normalized.startswith("${{") and normalized.endswith("}}"):
        normalized = normalized[3:-2].strip()
    return normalized in {"false", "0", "!true", "! true"}


def check_workflow_operation_authority(
    document: dict[str, Any],
    job: dict[str, Any],
    channel: dict[str, Any],
    workflow: str,
    owner: str,
) -> None:
    kind = channel["kind"]
    if kind in PROTECTED_PUBLICATION_KINDS:
        expected_environment = channel.get("environment")
        if not isinstance(expected_environment, str) or not expected_environment:
            fail(workflow, f"{owner}: {kind} operation must declare a protected GitHub Environment")
        if workflow_job_environment(job) != expected_environment:
            fail(
                workflow,
                f"{owner}: {kind} operation requires GitHub Environment {expected_environment!r}",
            )

    required_permission = {
        "github-release-assets": ("contents", "write"),
        "npm": ("id-token", "write"),
        "pypi": ("id-token", "write"),
        "pub.dev": ("id-token", "write"),
    }.get(kind)
    if required_permission is not None:
        job_permissions = job.get("permissions")
        permissions = (
            job_permissions
            if job.get("permissions_declared") is True and isinstance(job_permissions, dict)
            else document.get("permissions", {})
        )
        name, expected = required_permission
        if not isinstance(permissions, dict) or permissions.get(name) != expected:
            fail(workflow, f"{owner}: {kind} operation requires {name}: {expected}")

    if kind == "npm" and web_package_group_reconcile_steps(job):
        check_web_package_group_publish_boundary(job, workflow, owner)

    required_env = {
        "crates.io": {"CARGO_REGISTRY_TOKEN"},
        "github-release-assets": {"GH_REPO", "GH_TOKEN"},
    }.get(kind)
    if required_env is None:
        return
    job_env = job.get("env") if isinstance(job.get("env"), dict) else {}
    trusted_values = {
        "CARGO_REGISTRY_TOKEN": {"${{ secrets.CARGO_REGISTRY_TOKEN }}"},
        "GH_REPO": {"${{ github.repository }}"},
        "GH_TOKEN": {"${{ github.token }}", "${{ secrets.GITHUB_TOKEN }}"},
    }
    operation_steps = active_operation_steps(job, kind)
    for step in operation_steps:
        step_env = step.get("env") if isinstance(step.get("env"), dict) else {}
        visible_env = {**job_env, **step_env}
        missing = [
            key
            for key in sorted(required_env)
            if not (
                isinstance(visible_env.get(key), str)
                and visible_env[key].strip()
                and visible_env[key] in trusted_values[key]
            )
        ]
        if missing:
            fail(
                workflow,
                f"{owner}: {kind} operation step requires credential environment keys "
                + ", ".join(missing)
                + " with trusted values",
            )


def web_package_group_reconcile_steps(job: dict[str, Any]) -> list[tuple[int, dict[str, Any]]]:
    result: list[tuple[int, dict[str, Any]]] = []
    for index, step in enumerate(job.get("steps", [])):
        if condition_is_always_false(step.get("if")):
            continue
        run = step.get("run")
        if isinstance(run, str) and shell_run_invokes_web_package_group_reconcile(run):
            result.append((index, step))
    return result


def check_web_package_group_publish_boundary(
    job: dict[str, Any],
    workflow: str,
    owner: str,
) -> None:
    reconciles = web_package_group_reconcile_steps(job)
    if len(reconciles) != 1:
        fail(workflow, f"{owner}: expected exactly one Web package-group reconcile step")
    reconcile_index, _reconcile = reconciles[0]
    trusted_checkout = False
    for index, step in enumerate(job.get("steps", [])):
        run = step.get("run")
        if isinstance(run, str) and "target/npm-package-group/web_package_group.py" in run:
            fail(
                workflow,
                f"{owner}: publish job must not execute code supplied by the downloaded package artifact",
            )
        if index >= reconcile_index:
            continue
        uses = step.get("uses")
        if not isinstance(uses, str) or uses.partition("@")[0] != "actions/checkout":
            continue
        values = step.get("with") if isinstance(step.get("with"), dict) else {}
        if (
            values.get("ref") == "${{ github.workflow_sha }}"
            and values.get("persist-credentials") in {False, "false"}
        ):
            trusted_checkout = True
    if not trusted_checkout:
        fail(
            workflow,
            f"{owner}: package-group publish must checkout github.workflow_sha without credentials before reconciliation",
        )


def workflow_job_environment(job: dict[str, Any]) -> str | None:
    environment = job.get("environment")
    if isinstance(environment, str):
        return environment
    if isinstance(environment, dict):
        name = environment.get("name")
        if isinstance(name, str):
            return name
    return None


def active_operation_steps(job: dict[str, Any], kind: str) -> list[dict[str, Any]]:
    command_rules: dict[str, tuple[tuple[str, ...], ...]] = {
        "crates.io": (("cargo", "publish"),),
        "github-release-assets": (("gh", "release", "create"), ("gh", "release", "upload")),
        "homebrew": (("brew", "install"),),
        "npm": (("npm", "publish"),),
        "pub.dev": (("dart", "pub", "publish"),),
    }
    action_rules = {
        "github-actions-artifact": "actions/upload-artifact",
        "pypi": "pypa/gh-action-pypi-publish",
    }
    result: list[dict[str, Any]] = []
    for step in job.get("steps", []):
        if condition_is_always_false(step.get("if")):
            continue
        if kind in action_rules:
            uses = step.get("uses")
            if isinstance(uses, str) and uses.partition("@")[0] == action_rules[kind]:
                result.append(step)
        elif kind in command_rules:
            run = step.get("run")
            if isinstance(run, str) and operation_run_matches_kind(run, kind, command_rules[kind]):
                result.append(step)
    return result


def check_actions_artifact_contract(
    job: dict[str, Any],
    surface: dict[str, Any],
    channel: dict[str, Any],
    workflow: str,
    owner: str,
) -> None:
    upload_steps = [
        step
        for step in job.get("steps", [])
        if isinstance(step.get("uses"), str)
        and step["uses"].partition("@")[0] == "actions/upload-artifact"
    ]
    if len(upload_steps) != 1:
        fail(workflow, f"{owner}: expected exactly one artifact upload step")
    artifact_name = upload_steps[0].get("with", {}).get("name")
    if not isinstance(artifact_name, str):
        fail(workflow, f"{owner}: artifact upload must declare a name")

    template = normalize_actions_artifact_name(artifact_name)
    required_placeholders = ["{version}", "{channel}", "{source_sha}", "{target}"]
    manifest_version_packages = [
        package
        for package in surface.get("packages", [])
        if package.get("version_source", "target") == "manifest"
    ]
    if manifest_version_packages:
        if len(manifest_version_packages) != 1:
            fail(workflow, f"{owner}: artifact package_version source must be unambiguous")
        required_placeholders.append("{package_version}")
    elif "{package_version}" in template:
        fail(workflow, f"{owner}: package_version requires a manifest-version package")

    for placeholder in required_placeholders:
        if template.count(placeholder) != 1:
            fail(
                workflow,
                f"{owner}: artifact upload name must bind exactly one {placeholder}",
            )
    if "${{" in template:
        fail(workflow, f"{owner}: artifact upload name contains an unmodeled expression")

    prefix, suffix = template.split("{target}")
    targets: list[str] = []
    for pattern in channel["artifact_patterns"]:
        glob = pattern["glob"]
        if pattern["min_matches"] != 1 or pattern["max_matches"] != 1:
            fail(workflow, f"{owner}: each artifact target must match exactly once")
        if not glob.startswith(prefix) or (suffix and not glob.endswith(suffix)):
            fail(
                workflow,
                f"{owner}: artifact pattern {glob!r} does not match upload template {template!r}",
            )
        target_end = len(glob) - len(suffix) if suffix else len(glob)
        target = glob[len(prefix) : target_end]
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", target):
            fail(workflow, f"{owner}: artifact pattern has an invalid target {target!r}")
        targets.append(target)
    if len(targets) != len(set(targets)):
        fail(workflow, f"{owner}: artifact target patterns must be unique")

    matrix_rows = job.get("matrix_include")
    if not isinstance(matrix_rows, list):
        fail(workflow, f"{owner}: target-bound artifacts require a matrix include list")
    matrix_targets = [row.get("target") for row in matrix_rows]
    if not all(isinstance(target, str) and target for target in matrix_targets):
        fail(workflow, f"{owner}: every matrix row must declare target")
    if set(matrix_targets) != set(targets) or len(matrix_targets) != len(targets):
        fail(
            workflow,
            f"{owner}: artifact patterns and workflow matrix targets differ",
        )


def normalize_actions_artifact_name(name: str) -> str:
    normalized = re.sub(
        r"\$\{\{\s*steps\.[A-Za-z0-9_-]+\.outputs\.(?:extension|package)_version\s*\}\}",
        "{package_version}",
        name,
    )
    normalized = re.sub(
        r"\$\{\{\s*steps\.[A-Za-z0-9_-]+\.outputs\.(?:(?:release|runtime)_)?version\s*\}\}",
        "{version}",
        normalized,
    )
    normalized = re.sub(
        r"\$\{\{\s*steps\.[A-Za-z0-9_-]+\.outputs\.(?:(?:release|runtime)_)?channel\s*\}\}",
        "{channel}",
        normalized,
    )
    normalized = re.sub(
        r"\$\{\{\s*steps\.[A-Za-z0-9_-]+\.outputs\.source_sha\s*\}\}",
        "{source_sha}",
        normalized,
    )
    return re.sub(
        r"\$\{\{\s*matrix\.target\s*\}\}",
        "{target}",
        normalized,
    )


def check_cargo_dist_asset_contract(root: Path, contract: dict[str, Any]) -> None:
    dist_config = read_toml(root, "dist-workspace.toml").get("dist")
    if not isinstance(dist_config, dict):
        fail("dist-workspace.toml", "missing [dist] configuration")
    packages = require_nonempty_string_list(dist_config, "packages", "dist-workspace.toml")
    targets = require_nonempty_string_list(dist_config, "targets", "dist-workspace.toml")
    installers = require_nonempty_string_list(dist_config, "installers", "dist-workspace.toml")

    installer_suffixes = {"shell": "installer.sh", "powershell": "installer.ps1"}
    unknown_installers = set(installers) - set(installer_suffixes)
    if unknown_installers:
        fail(
            "dist-workspace.toml",
            "release verifier does not model installers: " + ", ".join(sorted(unknown_installers)),
        )

    package_channels: dict[str, dict[str, Any]] = {}
    for surface in contract["surfaces"]:
        crate_names = {
            package["name"]
            for package in surface.get("packages", [])
            if package["kind"] == "crate"
        }
        release_channels = [
            channel
            for channel in surface["channels"]
            if channel["kind"] == "github-release-assets"
        ]
        for package in set(packages) & crate_names:
            if len(release_channels) != 1:
                fail(
                    "docs/release/SURFACES.json",
                    f"cargo-dist package {package} must have exactly one GitHub Release channel",
                )
            package_channels[package] = release_channels[0]

    if set(package_channels) != set(packages):
        missing = sorted(set(packages) - set(package_channels))
        fail(
            "docs/release/SURFACES.json",
            "cargo-dist packages lack GitHub Release contracts: " + ", ".join(missing),
        )

    for package, channel in package_channels.items():
        archives = {
            f"{package}-{target}.{dist_archive_extension(target)}"
            for target in targets
        }
        expected = archives | {f"{archive}.sha256" for archive in archives}
        expected |= {f"{package}-{installer_suffixes[installer]}" for installer in installers}
        records = [
            pattern
            for pattern in channel["asset_patterns"]
            if pattern["glob"].startswith(f"{package}-")
        ]
        actual = {pattern["glob"] for pattern in records}
        if actual != expected:
            missing = sorted(expected - actual)
            extra = sorted(actual - expected)
            details = []
            if missing:
                details.append("missing " + ", ".join(missing))
            if extra:
                details.append("unexpected " + ", ".join(extra))
            fail(
                "docs/release/SURFACES.json",
                f"{package} cargo-dist asset contract differs: " + "; ".join(details),
            )
        if any(pattern["min_matches"] != 1 or pattern["max_matches"] != 1 for pattern in records):
            fail(
                "docs/release/SURFACES.json",
                f"{package} cargo-dist assets must each match exactly once",
            )


def require_nonempty_string_list(item: dict[str, Any], key: str, owner: str) -> list[str]:
    value = item.get(key)
    if (
        not isinstance(value, list)
        or not value
        or not all(isinstance(entry, str) and entry for entry in value)
        or len(value) != len(set(value))
    ):
        fail(owner, f"{key} must be a non-empty list of unique strings")
    return value


def dist_archive_extension(target: str) -> str:
    if target.endswith("-windows-msvc"):
        return "zip"
    if target.endswith(("-apple-darwin", "-unknown-linux-gnu")):
        return "tar.xz"
    fail("dist-workspace.toml", f"release verifier does not model target {target!r}")


def shell_run_invokes(run: str, expected: tuple[str, ...]) -> bool:
    functions, top_level_lines = shell_function_bodies(executable_shell_lines(run))
    pending_line_groups = [top_level_lines]
    visited_functions: set[str] = set()

    while pending_line_groups:
        for line in pending_line_groups.pop():
            tokens = shell_command_tokens(line)
            if tuple(tokens[: len(expected)]) == expected:
                return True
            if tokens and tokens[0] in functions and tokens[0] not in visited_functions:
                visited_functions.add(tokens[0])
                pending_line_groups.append(functions[tokens[0]])
    return False


SHELL_FUNCTION_DECLARATION = re.compile(
    r"^(?:function\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*(?:\(\s*\))?\s*\{\s*$"
)


def shell_function_bodies(lines: list[str]) -> tuple[dict[str, list[str]], list[str]]:
    functions: dict[str, list[str]] = {}
    return functions, shell_scope_lines(lines, functions)


def shell_scope_lines(lines: list[str], functions: dict[str, list[str]]) -> list[str]:
    scope: list[str] = []
    index = 0
    control_depth = 0

    while index < len(lines):
        line = lines[index]
        control_depth = max(0, control_depth - shell_control_blocks_closed(line))
        match = SHELL_FUNCTION_DECLARATION.fullmatch(line)
        if match is None:
            if control_depth == 0 and shell_line_terminates_scope(line):
                break
            scope.append(line)
            control_depth += shell_control_blocks_opened(line)
            index += 1
            continue

        name = match.group(1)
        body, next_index = shell_function_body(lines, index + 1)
        if body is None:
            # A malformed function cannot prove that it invokes a release operation.
            break
        functions[name] = shell_scope_lines(body, functions)
        index = next_index

    return scope


def shell_control_blocks_opened(line: str) -> int:
    if re.match(r"^(?:if|for|select|until|while)\b", line):
        return 0 if re.search(r"(?:^|;)\s*(?:fi|done)\s*;?\s*$", line) else 1
    if re.match(r"^case\b", line):
        return 0 if re.search(r"(?:^|;)\s*esac\s*;?\s*$", line) else 1
    if line == "(":
        return 1
    return 0


def shell_control_blocks_closed(line: str) -> int:
    return int(bool(re.match(r"^(?:fi|done|esac)\b", line) or line == ")"))


def shell_line_terminates_scope(line: str) -> bool:
    tokens = shell_command_tokens(line)
    return bool(tokens and tokens[0] in {"exit", "return"})


def shell_function_body(lines: list[str], start: int) -> tuple[list[str] | None, int]:
    depth = 1
    body: list[str] = []
    index = start
    while index < len(lines):
        line = lines[index]
        if line == "}":
            depth -= 1
            if depth == 0:
                return body, index + 1
            body.append(line)
        else:
            if line == "{" or SHELL_FUNCTION_DECLARATION.fullmatch(line):
                depth += 1
            body.append(line)
        index += 1
    return None, index


def shell_command_tokens(line: str) -> list[str]:
    if not line or line.startswith("#"):
        return []
    try:
        tokens = shlex.split(line, comments=True, posix=True)
    except ValueError:
        return []
    index = 0
    while index < len(tokens) and tokens[index] in {
        "{",
        "!",
        "if",
        "then",
        "elif",
        "do",
        "command",
        "exec",
    }:
        index += 1
    while index < len(tokens) and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", tokens[index]):
        index += 1
    if index < len(tokens) and tokens[index] == "env":
        index += 1
        while index < len(tokens):
            token = tokens[index]
            if token in {"-u", "--unset"} and index + 1 < len(tokens):
                index += 2
            elif token.startswith("-") or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", token):
                index += 1
            else:
                break
    return tokens[index:]


def executable_shell_lines(run: str) -> list[str]:
    lines: list[str] = []
    heredoc_delimiter: str | None = None
    disabled_if_depth = 0
    for raw_line in run.splitlines():
        line = raw_line.strip()
        if heredoc_delimiter is not None:
            if line == heredoc_delimiter:
                heredoc_delimiter = None
            continue
        if disabled_if_depth:
            if re.match(r"^if\b", line):
                disabled_if_depth += 1
            if re.match(r"^fi(?:\s*;|\s*$)", line):
                disabled_if_depth -= 1
            continue
        if re.match(r"^if\s+(?:command\s+)?false(?:\s*;)?\s*then(?:\s|;|$)", line):
            if not re.search(r"(?:^|;)\s*fi(?:\s*;|\s*$)", line):
                disabled_if_depth = 1
            continue
        heredoc = re.search(r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1", line)
        if heredoc is not None:
            heredoc_delimiter = heredoc.group(2)
        if line and not line.startswith("#"):
            lines.append(line)
    return lines


def check_package_manifest_names(root: Path, contract: dict[str, Any]) -> None:
    seen_names: set[tuple[str, str]] = set()
    seen_manifests: set[str] = set()
    for surface in contract["surfaces"]:
        for package in surface.get("packages", []):
            kind = package["kind"]
            name = package["name"]
            manifest = package["manifest"]
            key = (kind, name)
            if key in seen_names:
                fail(manifest, f"duplicate {kind} package declaration for {name}")
            if manifest in seen_manifests:
                fail(manifest, "package manifest is declared by more than one surface")
            seen_names.add(key)
            seen_manifests.add(manifest)
            actual = package_manifest_name(root, kind, manifest)
            if actual != name:
                fail(manifest, f"{kind} package name is {actual!r}, expected {name!r}")


def check_package_publishability(root: Path, contract: dict[str, Any]) -> None:
    for surface in contract["surfaces"]:
        channel_kinds = {channel["kind"] for channel in surface["channels"]}
        for package in surface.get("packages", []):
            kind = package["kind"]
            manifest = package["manifest"]
            if kind == "crate" and "crates.io" in channel_kinds:
                data = read_toml(root, manifest)
                publish = data["package"].get("publish")
                if publish is False or publish == [] or (
                    isinstance(publish, list) and "crates-io" not in publish
                ):
                    fail(manifest, "contract declares crates.io publication but manifest disables it")
            elif kind == "npm" and "npm" in channel_kinds:
                if read_json(root, manifest).get("private") is True:
                    fail(manifest, "contract declares npm publication but manifest sets private: true")
            elif kind == "flutter" and "pub.dev" in channel_kinds:
                if re.search(r"^publish_to:\s*['\"]?none['\"]?\s*$", read_text(root, manifest), re.MULTILINE):
                    fail(manifest, "contract declares pub.dev publication but manifest sets publish_to: none")


def check_package_inventory(root: Path, contract: dict[str, Any]) -> None:
    declared_manifests = {
        normalize_rel(package["manifest"])
        for surface in contract["surfaces"]
        for package in surface.get("packages", [])
    }
    package_jsons = {normalize_rel(path.relative_to(root)) for path in iter_package_jsons(root)}
    dynamic_non_surfaces = web_non_surface_package_manifests(root, contract)
    non_surface_manifests = NON_SURFACE_PACKAGE_MANIFESTS | dynamic_non_surfaces
    undeclared = sorted(
        package_jsons
        - declared_manifests
        - non_surface_manifests
    )
    if undeclared:
        fail(
            "docs/release/SURFACES.json",
            "package.json manifests are neither release surfaces nor allowlisted non-surfaces: "
            + ", ".join(undeclared),
        )

    for rel_path in sorted(REQUIRED_NON_SURFACE_PACKAGE_MANIFESTS):
        manifest = root / rel_path
        if not manifest.exists():
            fail(rel_path, "allowlisted non-surface package manifest is missing")

    for rel_path in sorted(non_surface_manifests):
        manifest = root / rel_path
        if manifest.exists() and rel_path != "package.json":
            data = json.loads(manifest.read_text(encoding="utf-8"))
            if data.get("private") is not True:
                fail(rel_path, "non-surface package manifest must set private: true")


def web_non_surface_package_manifests(root: Path, contract: dict[str, Any]) -> set[str]:
    feature_contract = contract.get("feature_contract")
    if not isinstance(feature_contract, dict):
        return set()
    descriptor_path = feature_contract.get("web_descriptor")
    if descriptor_path != WEB_SURFACE_DESCRIPTOR_PATH:
        return set()
    candidate = root / descriptor_path
    if not candidate.is_file():
        return set()
    descriptor = validate_web_surface_descriptor(read_json(root, descriptor_path), descriptor_path)
    web_package_group = load_web_package_group_module()
    manifests = {"platforms/web/package.json"}
    for entry in descriptor["packages"]:
        if entry["visibility"] == "candidate":
            manifests.add(
                normalize_rel(
                    Path("platforms/web")
                    / web_package_group.descriptor_package_path(entry)
                    / "package.json"
                )
            )
    return manifests


def iter_package_jsons(root: Path) -> list[Path]:
    git_manifests = git_visible_package_jsons(root)
    if git_manifests is not None:
        return git_manifests

    manifests: list[Path] = []
    for current, dirs, files in os.walk(root, followlinks=False):
        current_path = Path(current)
        dirs[:] = sorted(
            name
            for name in dirs
            if name not in PACKAGE_INVENTORY_IGNORED_DIRECTORY_NAMES
            and normalize_rel((current_path / name).relative_to(root))
            not in PACKAGE_INVENTORY_IGNORED_RELATIVE_DIRECTORIES
        )
        if "package.json" in files:
            manifests.append(current_path / "package.json")
    return sorted(manifests)


def git_visible_package_jsons(root: Path) -> list[Path] | None:
    """Return tracked and non-ignored untracked manifests for a Git repository."""
    try:
        top_level = subprocess.run(
            ["git", "-C", str(root), "rev-parse", "--show-toplevel"],
            check=False,
            capture_output=True,
            text=True,
        )
    except OSError:
        return None
    if top_level.returncode != 0:
        return None
    try:
        if Path(top_level.stdout.strip()).resolve(strict=True) != root.resolve(strict=True):
            return None
    except OSError:
        return None

    try:
        result = subprocess.run(
            [
                "git",
                "-C",
                str(root),
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
                "--",
                "package.json",
                "**/package.json",
            ],
            check=False,
            capture_output=True,
        )
    except OSError as error:
        raise CheckFailure(f"git package manifest inventory failed: {error}") from error
    if result.returncode != 0:
        diagnostic = os.fsdecode(result.stderr).strip() or "git ls-files failed"
        raise CheckFailure(f"git package manifest inventory failed: {diagnostic}")

    manifests = {
        root / os.fsdecode(raw_path)
        for raw_path in result.stdout.split(b"\0")
        if raw_path and Path(os.fsdecode(raw_path)).name == "package.json"
    }
    return sorted(path for path in manifests if path.is_file())


def check_publishable_crate_inventory(root: Path, contract: dict[str, Any]) -> None:
    metadata = cargo_metadata(root)
    publishable = {
        package["name"]
        for package in metadata["packages"]
        if cargo_package_is_publishable(package)
    }
    declared = {
        package["name"]
        for surface in contract["surfaces"]
        for package in surface.get("packages", [])
        if package["kind"] == "crate"
    }
    if declared != publishable:
        missing = sorted(publishable - declared)
        extra = sorted(declared - publishable)
        details = []
        if missing:
            details.append("missing publishable crates: " + ", ".join(missing))
        if extra:
            details.append("declared non-publishable crates: " + ", ".join(extra))
        fail("docs/release/SURFACES.json", "; ".join(details))

    for surface in contract["surfaces"]:
        crates = [package["name"] for package in surface.get("packages", []) if package["kind"] == "crate"]
        if crates and not any(channel["kind"] == "crates.io" for channel in surface["channels"]):
            fail(
                "docs/release/SURFACES.json",
                f"{surface['id']}: crate packages lack a crates.io channel: {', '.join(crates)}",
            )


def cargo_metadata(root: Path) -> dict[str, Any]:
    try:
        result = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            cwd=root,
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(result.stdout)
    except (OSError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        raise CheckFailure(f"cargo metadata failed: {error}") from error


def cargo_package_is_publishable(package: dict[str, Any]) -> bool:
    publish = package.get("publish")
    return publish is None or publish is True or (
        isinstance(publish, list) and "crates-io" in publish
    )


def check_web_contract(root: Path, contract: dict[str, Any]) -> None:
    feature_contract = contract["feature_contract"]
    descriptor = load_web_surface_descriptor(root, feature_contract)
    web_package_group = load_web_package_group_module()
    try:
        web_package_group.validate_workspace_manifest(root)
    except web_package_group.PackageGroupError as error:
        fail("platforms/web/package.json", str(error))

    profile_ids = artifact_profile_ids(root)
    for entry in descriptor["packages"]:
        if entry["artifact_profile"] not in profile_ids:
            fail(
                feature_contract["web_descriptor"],
                f"package {entry['id']} references missing artifact profile {entry['artifact_profile']}",
            )
        package_dir = root / "platforms" / "web" / web_package_group.descriptor_package_path(entry)
        try:
            web_package_group.validate_package_manifest(entry, package_dir, expected_version=None)
        except web_package_group.PackageGroupError as error:
            fail(package_dir / "package.json", str(error))

    package_versions: set[str] = set()
    for entry in descriptor["packages"]:
        manifest_path = Path("platforms/web") / web_package_group.descriptor_package_path(entry) / "package.json"
        version = read_json(root, str(manifest_path)).get("version")
        if not isinstance(version, str) or not version:
            fail(manifest_path, "Web package version must be a non-empty string")
        package_versions.add(version)
    if len(package_versions) != 1:
        fail(feature_contract["web_descriptor"], "all Web package manifests must share one version")

    group_surface_id = feature_contract["web_package_group_surface"]
    group_surface = next(
        (surface for surface in contract["surfaces"] if surface["id"] == group_surface_id),
        None,
    )
    if group_surface is None:
        fail("docs/release/SURFACES.json", f"missing Web package group surface {group_surface_id!r}")
    declared_npm = {
        package["name"]: package["manifest"]
        for package in group_surface["packages"]
        if package["kind"] == "npm"
    }
    expected_npm = {
        entry["name"]: str(
            Path("platforms/web")
            / web_package_group.descriptor_package_path(entry)
            / "package.json"
        )
        for entry in web_package_group.public_packages(descriptor)
    }
    if declared_npm != expected_npm:
        fail(
            "docs/release/SURFACES.json",
            "Web npm release group must exactly match public descriptor packages",
        )
    candidate_names = {
        entry["name"] for entry in descriptor["packages"] if entry["visibility"] == "candidate"
    }
    all_declared_npm = {
        package["name"]
        for surface in contract["surfaces"]
        for package in surface["packages"]
        if package["kind"] == "npm"
    }
    leaked_candidates = sorted(candidate_names & all_declared_npm)
    if leaked_candidates:
        fail(
            "docs/release/SURFACES.json",
            "candidate Web packages must not enter a release contract: " + ", ".join(leaked_candidates),
        )


def artifact_profile_ids(root: Path) -> set[str]:
    descriptor = read_json(root, ARTIFACT_PROFILES_PATH)
    profiles = descriptor.get("profiles")
    if not isinstance(profiles, list):
        fail(ARTIFACT_PROFILES_PATH, "profiles must be an array")
    ids = {profile.get("id") for profile in profiles if isinstance(profile, dict)}
    if not all(isinstance(profile_id, str) and profile_id for profile_id in ids):
        fail(ARTIFACT_PROFILES_PATH, "profiles must have string ids")
    if len(ids) != len(profiles):
        fail(ARTIFACT_PROFILES_PATH, "profiles must have unique ids")
    return ids


def render_public_surface_table(contract: dict[str, Any]) -> str:
    lines = [
        GENERATED_SURFACES_BEGIN,
        "| Contract ID | Surface | Entry point | Support | Channels |",
        "| --- | --- | --- | --- | --- |",
    ]
    for surface in contract["surfaces"]:
        if not surface["public"]:
            continue
        channels = ", ".join(
            f"`{channel['id']}` (`{channel['declared_state']}`)"
            for channel in surface["channels"]
        )
        lines.append(
            f"| `{surface['id']}` | {surface['name']} | `{surface['entry_point']}` | "
            f"`{surface['support_level']}` | {channels} |"
        )
    lines.append(GENERATED_SURFACES_END)
    return "\n".join(lines)


def generated_markdown_block(text: str, begin: str, end: str) -> str:
    if text.count(begin) != 1 or text.count(end) != 1:
        fail(
            "docs/release/PACKAGE_SURFACES.md",
            f"expected exactly one generated block bounded by {begin!r} and {end!r}",
        )
    prefix, remainder = text.split(begin, 1)
    del prefix
    body, _suffix = remainder.split(end, 1)
    return begin + body.rstrip() + "\n" + end


def write_generated_surface_docs(root: Path, contract: dict[str, Any]) -> None:
    rel_path = "docs/release/PACKAGE_SURFACES.md"
    path = require_file(root, rel_path)
    text = path.read_text(encoding="utf-8")
    current = generated_markdown_block(text, GENERATED_SURFACES_BEGIN, GENERATED_SURFACES_END)
    expected = render_public_surface_table(contract)
    if current != expected:
        path.write_text(text.replace(current, expected), encoding="utf-8")


def check_blocked_channel_metadata(contract: dict[str, Any]) -> None:
    for surface in contract["surfaces"]:
        for channel in surface.get("channels", []):
            state = channel["declared_state"]
            owner = f"docs/release/SURFACES.json:{surface['id']}/{channel['id']}"
            if state == "credential-blocked" and not channel.get("credential"):
                fail(owner, "credential-blocked channels must name the missing credential")
            if state in {"credential-blocked", "registry-blocked", "manual-registry"} and not channel.get("blocker"):
                fail(owner, f"{state} channels must explain the blocker")
            if state == "not-applicable" and not channel.get("not_applicable_reason"):
                fail(owner, "not-applicable channels must explain why")
            if set(channel.get("release_kinds", [])) != {"stable", "prerelease"} and not channel.get(
                "not_applicable_reason"
            ):
                fail(owner, "conditionally not-applicable channels must explain why")


def check_ci_wiring(root: Path) -> None:
    workflow_contract = load_workflow_contract_module()
    workflow_path = ".github/workflows/ci.yml"
    try:
        document = workflow_contract.load_workflow_contract(require_file(root, workflow_path))
    except (OSError, workflow_contract.WorkflowContractError) as error:
        fail(workflow_path, f"invalid CI workflow contract: {error}")

    active_steps = [
        step
        for job in document["jobs"].values()
        if not condition_is_always_false(job.get("if"))
        for step in job.get("steps", [])
        if not condition_is_always_false(step.get("if"))
        and isinstance(step.get("run"), str)
    ]
    verifier = ("python3", "scripts/verify-release-surfaces.py")
    if not any(shell_run_invokes(step["run"], verifier) for step in active_steps):
        fail(workflow_path, f"CI does not execute {' '.join(verifier)}")
    for test_script in [
        "scripts/test_release_status.py",
        "scripts/test_verify_release_surfaces.py",
        "scripts/test_web_package_group.py",
    ]:
        if not any(
            shell_run_invokes_python_unittest(step["run"], test_script)
            for step in active_steps
        ):
            fail(workflow_path, f"CI does not execute unittest module {test_script}")


def shell_run_invokes_python_unittest(run: str, test_script: str) -> bool:
    logical_run = run.replace("\\\n", " ")
    for line in executable_shell_lines(logical_run):
        tokens = shell_command_tokens(line)
        if tokens[:3] == ["python3", "-m", "unittest"] and test_script in tokens[3:]:
            return True
    return False


def package_manifest_name(root: Path, kind: str, manifest: str) -> str:
    if kind in {"npm", "vscode"}:
        return require_manifest_string(read_json(root, manifest), manifest, "name")
    if kind == "crate":
        return require_manifest_string(read_toml(root, manifest), manifest, "package", "name")
    if kind == "python":
        return require_manifest_string(read_toml(root, manifest), manifest, "project", "name")
    if kind == "flutter":
        return require_regex(manifest, read_text(root, manifest), r"^name:\s*([^\s#]+)")
    if kind == "typst":
        return require_manifest_string(read_toml(root, manifest), manifest, "package", "name")
    if kind == "android":
        text = strip_c_style_comments(read_text(root, manifest))
        group = require_regex(manifest, text, r"^\s*group\s*=\s*\"([^\"]+)\"")
        artifact = require_regex(manifest, text, r"^\s*artifactId\s*=\s*\"([^\"]+)\"")
        return f"{group}:{artifact}"
    if kind == "swiftpm":
        text = strip_c_style_comments(read_text(root, manifest))
        return require_regex(
            manifest,
            text,
            r"\blet\s+package\s*=\s*Package\s*\(\s*name\s*:\s*\"([^\"]+)\"",
        )
    raise CheckFailure(f"unsupported package kind {kind!r} in {manifest}")


def require_manifest_string(data: Any, manifest: str, *keys: str) -> str:
    current = data
    for key in keys:
        if not isinstance(current, dict) or key not in current:
            fail(manifest, f"missing manifest field {'.'.join(keys)}")
        current = current[key]
    if not isinstance(current, str) or not current:
        fail(manifest, f"manifest field {'.'.join(keys)} must be a non-empty string")
    return current


def strip_c_style_comments(text: str) -> str:
    result: list[str] = []
    index = 0
    quote: str | None = None
    block_depth = 0
    line_comment = False
    while index < len(text):
        char = text[index]
        following = text[index + 1] if index + 1 < len(text) else ""
        if line_comment:
            if char == "\n":
                line_comment = False
                result.append(char)
            else:
                result.append(" ")
            index += 1
            continue
        if block_depth:
            if char == "/" and following == "*":
                block_depth += 1
                result.extend((" ", " "))
                index += 2
            elif char == "*" and following == "/":
                block_depth -= 1
                result.extend((" ", " "))
                index += 2
            else:
                result.append("\n" if char == "\n" else " ")
                index += 1
            continue
        if quote is not None:
            result.append(char)
            if char == "\\" and following:
                result.append(following)
                index += 2
            else:
                if char == quote:
                    quote = None
                index += 1
            continue
        if char in {'"', "'"}:
            quote = char
            result.append(char)
            index += 1
        elif char == "/" and following == "/":
            line_comment = True
            result.extend((" ", " "))
            index += 2
        elif char == "/" and following == "*":
            block_depth = 1
            result.extend((" ", " "))
            index += 2
        else:
            result.append(char)
            index += 1
    return "".join(result)


def load_web_surface_descriptor(
    root: Path,
    feature_contract: dict[str, Any],
) -> dict[str, Any]:
    rel_path = feature_contract.get("web_descriptor")
    if not isinstance(rel_path, str) or not rel_path:
        fail("docs/release/SURFACES.json", "feature_contract.web_descriptor is required")
    if rel_path != WEB_SURFACE_DESCRIPTOR_PATH:
        fail(
            "docs/release/SURFACES.json",
            f"web_descriptor must be {WEB_SURFACE_DESCRIPTOR_PATH}",
        )
    require_file(root, rel_path)
    return validate_web_surface_descriptor(read_json(root, rel_path), rel_path)


def validate_web_surface_descriptor(
    descriptor: dict[str, Any],
    rel_path: str = WEB_SURFACE_DESCRIPTOR_PATH,
) -> dict[str, Any]:
    web_package_group = load_web_package_group_module()
    try:
        return web_package_group.validate_descriptor(descriptor)
    except web_package_group.PackageGroupError as error:
        fail(rel_path, str(error))


def require_file(root: Path, rel_path: str) -> Path:
    if not isinstance(rel_path, str) or not rel_path:
        fail("docs/release/SURFACES.json", "release surface path must be a non-empty string")
    relative = Path(rel_path)
    if relative.is_absolute() or ".." in relative.parts:
        fail(rel_path, "release surface path must stay relative to the repository")
    candidate = root / relative
    try:
        resolved_root = root.resolve(strict=True)
        resolved = candidate.resolve(strict=True)
    except OSError as error:
        fail(rel_path, f"required release surface file is missing: {error}")
    if resolved != resolved_root and resolved_root not in resolved.parents:
        fail(rel_path, "release surface path resolves outside the repository")
    if not resolved.is_file():
        fail(rel_path, "release surface path must name a regular file")
    return resolved


def read_text(root: Path, rel_path: str) -> str:
    return require_file(root, rel_path).read_text(encoding="utf-8")


def read_json(root: Path, rel_path: str) -> dict[str, Any]:
    try:
        return json.loads(read_text(root, rel_path), object_pairs_hook=reject_duplicate_json_keys)
    except json.JSONDecodeError as error:
        fail(rel_path, f"invalid JSON: {error}")


def read_toml(root: Path, rel_path: str) -> dict[str, Any]:
    try:
        return tomllib.loads(read_text(root, rel_path))
    except tomllib.TOMLDecodeError as error:
        fail(rel_path, f"invalid TOML: {error}")


def require_regex(rel_path: str, text: str, pattern: str) -> str:
    match = re.search(pattern, text, flags=re.MULTILINE)
    if not match:
        fail(rel_path, f"missing pattern {pattern}")
    return match.group(1)


def fail(path: str | Path, message: str) -> None:
    normalized = normalize_rel(path)
    raise CheckFailure(f"::error file={normalized}::{message}")


def rel(path: Path, root: Path) -> str:
    try:
        return normalize_rel(path.relative_to(root))
    except ValueError:
        return normalize_rel(path)


def normalize_rel(path: str | Path) -> str:
    return str(path).replace("\\", "/")


if __name__ == "__main__":
    raise SystemExit(main())

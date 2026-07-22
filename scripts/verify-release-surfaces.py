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
REQUIRED_SURFACE_DOCS = [
    "docs/release/PACKAGE_SURFACES.md",
    "docs/release/RELEASING.md",
    "docs/release/ADDING_SURFACE.md",
    "docs/release/MERMAID_UPGRADE_PLAYBOOK.md",
    "docs/security/RENDERING_SECURITY.md",
]
GENERATED_SURFACES_BEGIN = "<!-- BEGIN GENERATED RELEASE SURFACES -->"
GENERATED_SURFACES_END = "<!-- END GENERATED RELEASE SURFACES -->"
GENERATED_RELEASE_DOCS = {"docs/release/PACKAGE_SURFACES.md"}
WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION = 1
WEB_SURFACE_DESCRIPTOR_PATH = "platforms/web/web-surface-descriptor.json"
WEB_CAPABILITY_NAMES = {
    "render",
    "analysis",
    "ascii",
    "core_host",
    "elk_layout",
    "ratex_math",
    "editor_language",
}
WEB_RUNTIME_PROFILES = {"core", "render", "render-only", "ascii", "editor", "full"}
EVIDENCE_ONLY_WEB_PRESETS = {
    "browser-bridge",
    "browser-full-no-elk",
    "browser-ratex-math",
}
WEB_CAPABILITY_FEATURES = {
    "render": "render",
    "analysis": "analysis",
    "ascii": "ascii",
    "core_host": "core-host",
    "elk_layout": "elk-layout",
    "ratex_math": "ratex-math",
    "editor_language": "editor-language",
}
WEB_RUNTIME_CAPABILITIES = {
    "core": {"analysis"},
    "render": {"analysis", "render"},
    "render-only": {"render"},
    "ascii": {"ascii"},
    "editor": {"analysis", "editor_language"},
    "full": {"analysis", "ascii", "core_host", "editor_language", "elk_layout", "render"},
}
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
        ("release docs contract", lambda: check_release_docs(root, contract)),
        ("host text measurement docs", lambda: check_host_text_measurement_docs(root)),
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


def check_surface_paths(root: Path, contract: dict[str, Any]) -> None:
    require_file(root, "docs/release/SURFACES.json")
    for doc in REQUIRED_SURFACE_DOCS:
        require_file(root, doc)

    for doc in contract.get("feature_contract", {}).get("docs", []):
        require_file(root, doc)

    for surface in contract["surfaces"]:
        for doc in surface.get("docs", []):
            require_file(root, doc)
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
            and any(shell_run_invokes(step["run"], command) for command in command_rules[kind])
            for step in steps
        )
    raise CheckFailure(f"no workflow operation rule for operational channel kind {kind!r}")


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
            if isinstance(run, str) and any(
                shell_run_invokes(run, command) for command in command_rules[kind]
            ):
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
    undeclared = sorted(
        package_jsons
        - declared_manifests
        - NON_SURFACE_PACKAGE_MANIFESTS
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

    for rel_path in sorted(NON_SURFACE_PACKAGE_MANIFESTS):
        manifest = root / rel_path
        if manifest.exists() and rel_path != "package.json":
            data = json.loads(manifest.read_text(encoding="utf-8"))
            if data.get("private") is not True:
                fail(rel_path, "non-surface package manifest must set private: true")


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
    web_package = read_json(root, "platforms/web/package.json")
    actual_exports = web_package.get("exports")
    expected_exports = expected_web_package_exports(descriptor, feature_contract)
    if actual_exports != expected_exports:
        actual_keys = set(actual_exports) if isinstance(actual_exports, dict) else set()
        expected_keys = set(expected_exports)
        missing = sorted(expected_keys - actual_keys)
        extra = sorted(actual_keys - expected_keys)
        wrong = sorted(
            key
            for key in expected_keys & actual_keys
            if actual_exports[key] != expected_exports[key]
        )
        details = []
        if missing:
            details.append("missing: " + ", ".join(missing))
        if extra:
            details.append("unexpected: " + ", ".join(extra))
        if wrong:
            details.append("wrong targets: " + ", ".join(wrong))
        fail(
            "platforms/web/package.json",
            "Web exports must exactly match the descriptor and release contract ("
            + "; ".join(details)
            + ")",
        )

    expected_presets = set(feature_contract["browser_presets"])
    descriptor_presets = {preset["name"] for preset in descriptor["presets"]}
    if descriptor_presets != expected_presets:
        fail(
            feature_contract["web_descriptor"],
            "browser preset mismatch: expected "
            + ", ".join(sorted(expected_presets))
            + "; found "
            + ", ".join(sorted(descriptor_presets)),
        )

    public_surfaces = descriptor["public_surfaces"]
    descriptor_subpaths = {"."} | {f"./{surface['entry']}" for surface in public_surfaces}
    public_presets = {surface["preset"] for surface in public_surfaces}
    required_public_presets = expected_presets - EVIDENCE_ONLY_WEB_PRESETS
    if public_presets != required_public_presets:
        fail(
            feature_contract["web_descriptor"],
            "public surfaces should cover shipped presets only: "
            + ", ".join(sorted(public_presets)),
        )
    expected_default = feature_contract["web_default_preset"]
    if descriptor["default_preset"] != expected_default:
        fail(
            feature_contract["web_descriptor"],
            f"default preset is {descriptor['default_preset']!r}, expected {expected_default!r}",
        )

    wasm_features = cargo_features(root, "crates/merman-wasm/Cargo.toml")
    for feature in ["core-host", "analysis", "ascii", "render", "cytoscape-layout", "elk-layout", "editor-language", "ratex-math"]:
        if feature not in wasm_features:
            fail("crates/merman-wasm/Cargo.toml", f"missing wasm feature {feature}")
    for preset in descriptor["presets"]:
        for feature in preset["features"]:
            if feature not in wasm_features:
                fail(
                    feature_contract["web_descriptor"],
                    f"preset {preset['name']} references missing wasm feature {feature}",
                )
    validate_web_preset_capabilities(root, descriptor, feature_contract["web_descriptor"])
    validate_web_runtime_profiles(descriptor, feature_contract["web_descriptor"])

    web_docs = "\n".join(
        [
            read_text(root, "README.md"),
            read_text(root, "platforms/web/README.md"),
            read_text(root, "docs/release/PACKAGE_SURFACES.md"),
        ]
    )
    for surface in public_surfaces:
        term = f"@mermanjs/web/{surface['entry']}"
        if term not in web_docs:
            fail("docs/release/PACKAGE_SURFACES.md", f"missing web subpath docs for {term}")
    if "@mermanjs/web/analysis" in web_docs and "no `@mermanjs/web/analysis`" not in web_docs:
        fail("docs/release/PACKAGE_SURFACES.md", "analysis must be documented as absent, not as a package")

    if "./analysis" in descriptor_subpaths:
        fail(feature_contract["web_descriptor"], "analysis is not a supported public Web surface")


def expected_web_package_exports(
    descriptor: dict[str, Any],
    feature_contract: dict[str, Any],
) -> dict[str, Any]:
    auxiliary = feature_contract.get("web_auxiliary_exports")
    if not isinstance(auxiliary, dict) or not auxiliary:
        fail("docs/release/SURFACES.json", "feature_contract.web_auxiliary_exports is required")
    exports: dict[str, Any] = dict(auxiliary)
    for surface in descriptor["public_surfaces"]:
        entry = surface["entry"]
        exports[f"./{entry}"] = {
            "import": f"./dist/surfaces/{entry}.js",
            "types": f"./dist/surfaces/{entry}.d.ts",
        }

    package_dirs = ["pkg"] + [surface["pkg_dir_rel"] for surface in descriptor["public_surfaces"]]
    for package_dir in package_dirs:
        exports[f"./{package_dir}/merman_wasm.js"] = {
            "import": f"./{package_dir}/merman_wasm.js",
            "types": f"./{package_dir}/merman_wasm.d.ts",
        }
        exports[f"./{package_dir}/merman_wasm_bg.wasm"] = f"./{package_dir}/merman_wasm_bg.wasm"
    return exports


def validate_web_preset_capabilities(
    root: Path,
    descriptor: dict[str, Any],
    rel_path: str,
) -> None:
    feature_table = read_toml(root, "crates/merman-wasm/Cargo.toml").get("features", {})
    for preset in descriptor["presets"]:
        enabled = cargo_feature_closure(
            feature_table,
            preset["features"],
            include_defaults=preset["default_features"],
        )
        expected = {
            capability: feature in enabled
            for capability, feature in WEB_CAPABILITY_FEATURES.items()
        }
        if preset["capabilities"] != expected:
            fail(
                rel_path,
                f"preset {preset['name']} capabilities do not match its Cargo feature closure",
            )


def cargo_feature_closure(
    feature_table: dict[str, Any],
    features: list[str],
    *,
    include_defaults: bool,
) -> set[str]:
    pending = list(features)
    if include_defaults:
        pending.append("default")
    enabled: set[str] = set()
    while pending:
        feature = pending.pop()
        if feature in enabled:
            continue
        enabled.add(feature)
        for dependency in feature_table.get(feature, []):
            if dependency in feature_table:
                pending.append(dependency)
    return enabled


def validate_web_runtime_profiles(descriptor: dict[str, Any], rel_path: str) -> None:
    presets = {preset["name"]: preset for preset in descriptor["presets"]}
    for surface in descriptor["public_surfaces"]:
        profile = surface["runtime_profile"]
        preset = presets[surface["preset"]]
        actual = {
            capability
            for capability, enabled in preset["capabilities"].items()
            if enabled
        }
        expected = WEB_RUNTIME_CAPABILITIES[profile]
        if actual != expected:
            fail(
                rel_path,
                f"public surface {surface['entry']} maps profile {profile} to incompatible preset {surface['preset']}",
            )


def check_release_docs(root: Path, contract: dict[str, Any]) -> None:
    package_surfaces = read_text(root, "docs/release/PACKAGE_SURFACES.md")
    releasing = read_text(root, "docs/release/RELEASING.md")
    features = read_text(root, "docs/FEATURES.md")
    readme = read_text(root, "README.md")

    expected_surface_table = render_public_surface_table(contract)
    actual_surface_table = generated_markdown_block(
        package_surfaces,
        GENERATED_SURFACES_BEGIN,
        GENERATED_SURFACES_END,
    )
    if actual_surface_table != expected_surface_table:
        fail(
            "docs/release/PACKAGE_SURFACES.md",
            "generated release surface table is stale; run scripts/verify-release-surfaces.py --write-docs",
        )

    documented_states = markdown_state_names(package_surfaces)
    if documented_states != set(contract["states"]):
        fail(
            "docs/release/PACKAGE_SURFACES.md",
            "release status table must document exactly the contract state catalog",
        )

    for surface in contract["surfaces"]:
        if not surface["public"]:
            continue
        check_public_surface_entry_point_docs(root, surface)

    descriptor = load_web_surface_descriptor(root, contract["feature_contract"])
    documented_features = cargo_features(root, "crates/merman-wasm/Cargo.toml") - {"default"}
    documented_presets = {preset["name"] for preset in descriptor["presets"]}
    for term in sorted(documented_features | documented_presets):
        if term not in features + readme + package_surfaces:
            fail("docs/FEATURES.md", f"missing feature or preset name {term}")

    for command in [
        "scripts/release-status.py",
        "scripts/verify-release-surfaces.py",
    ]:
        if command not in releasing + package_surfaces:
            fail("docs/release/RELEASING.md", f"missing release helper command {command}")


def check_public_surface_entry_point_docs(root: Path, surface: dict[str, Any]) -> None:
    owner = f"docs/release/SURFACES.json:{surface['id']}"
    docs = [doc for doc in surface["docs"] if doc not in GENERATED_RELEASE_DOCS]
    if not docs:
        fail(owner, "public surface must declare at least one non-generated documentation file")
    entry_point = surface["entry_point"]
    if not any(entry_point in read_text(root, doc) for doc in docs):
        fail(owner, f"entry point {entry_point!r} is absent from declared non-generated docs")


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


def markdown_state_names(text: str) -> set[str]:
    match = re.search(
        r"^## Release Status States\s*$\n(?P<body>.*?)(?=^## |\Z)",
        text,
        flags=re.MULTILINE | re.DOTALL,
    )
    if match is None:
        fail("docs/release/PACKAGE_SURFACES.md", "missing Release Status States section")
    return set(re.findall(r"^\| `([^`]+)` \|", match.group("body"), flags=re.MULTILINE))


def write_generated_surface_docs(root: Path, contract: dict[str, Any]) -> None:
    rel_path = "docs/release/PACKAGE_SURFACES.md"
    path = require_file(root, rel_path)
    text = path.read_text(encoding="utf-8")
    current = generated_markdown_block(text, GENERATED_SURFACES_BEGIN, GENERATED_SURFACES_END)
    expected = render_public_surface_table(contract)
    if current != expected:
        path.write_text(text.replace(current, expected), encoding="utf-8")


def check_host_text_measurement_docs(root: Path) -> None:
    readme = read_text(root, "README.md")
    stale = "This surface does not expose host text-measurement callbacks yet"
    if stale in readme:
        fail("README.md", "Python row still says host text measurement is not exposed")

    for rel_path in [
        "README.md",
        "docs/bindings/HOST_TEXT_MEASUREMENT.md",
        "docs/bindings/PYTHON_UNIFFI.md",
        "platforms/python/merman/README.md",
    ]:
        text = read_text(root, rel_path)
        for token in ["MermanTextMeasurer", "reusable_engine_with_text_measurer"]:
            if token not in text:
                fail(rel_path, f"missing host text measurement token {token}")


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


def cargo_features(root: Path, manifest: str) -> set[str]:
    data = read_toml(root, manifest)
    return set(data.get("features", {}))


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
    require_exact_keys(
        descriptor,
        {"schema_version", "default_preset", "presets", "public_surfaces"},
        rel_path,
        "Web surface descriptor",
    )
    if descriptor["schema_version"] != WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION:
        fail(
            rel_path,
            f"Web surface descriptor schema must be {WEB_SURFACE_DESCRIPTOR_SCHEMA_VERSION}",
        )

    presets = descriptor["presets"]
    if not isinstance(presets, list) or not presets:
        fail(rel_path, "Web surface descriptor presets must be a non-empty array")
    preset_names: set[str] = set()
    for index, preset in enumerate(presets):
        label = f"presets[{index}]"
        require_exact_keys(
            preset,
            {"name", "surface", "default_features", "features", "capabilities"},
            rel_path,
            label,
        )
        name = require_web_name(preset["name"], rel_path, f"{label}.name")
        if name in preset_names:
            fail(rel_path, f"duplicate Web preset name: {name}")
        preset_names.add(name)
        if preset["surface"] != "browser":
            fail(rel_path, f"preset {name} must declare surface browser")
        if not isinstance(preset["default_features"], bool):
            fail(rel_path, f"preset {name} default_features must be boolean")
        features = preset["features"]
        if not isinstance(features, list):
            fail(rel_path, f"preset {name} features must be an array")
        normalized_features = [
            require_web_name(feature, rel_path, f"preset {name} feature")
            for feature in features
        ]
        if len(set(normalized_features)) != len(normalized_features):
            fail(rel_path, f"preset {name} contains duplicate features")
        capabilities = preset["capabilities"]
        require_exact_keys(
            capabilities,
            WEB_CAPABILITY_NAMES,
            rel_path,
            f"preset {name} capabilities",
        )
        for capability, enabled in capabilities.items():
            if not isinstance(enabled, bool):
                fail(rel_path, f"preset {name} capability {capability} must be boolean")

    default_preset = require_web_name(
        descriptor["default_preset"],
        rel_path,
        "default_preset",
    )
    if default_preset not in preset_names:
        fail(rel_path, f"default_preset references unknown preset {default_preset}")

    public_surfaces = descriptor["public_surfaces"]
    if not isinstance(public_surfaces, list) or not public_surfaces:
        fail(rel_path, "public_surfaces must be a non-empty array")
    entries: set[str] = set()
    public_presets: set[str] = set()
    package_dirs: set[str] = set()
    for index, surface in enumerate(public_surfaces):
        label = f"public_surfaces[{index}]"
        require_exact_keys(
            surface,
            {"entry", "preset", "pkg_dir_rel", "runtime_profile"},
            rel_path,
            label,
        )
        entry = require_web_name(surface["entry"], rel_path, f"{label}.entry")
        preset = require_web_name(surface["preset"], rel_path, f"surface {entry} preset")
        package_dir = surface["pkg_dir_rel"]
        if not isinstance(package_dir, str) or not re.fullmatch(
            r"pkg/[a-z0-9][a-z0-9-]*",
            package_dir,
        ):
            fail(rel_path, f"surface {entry} pkg_dir_rel must be a package-relative directory")
        runtime_profile = require_web_name(
            surface["runtime_profile"],
            rel_path,
            f"surface {entry} runtime_profile",
        )
        if entry in entries:
            fail(rel_path, f"duplicate public Web surface entry: {entry}")
        if preset in public_presets:
            fail(rel_path, f"duplicate public Web surface preset: {preset}")
        if package_dir in package_dirs:
            fail(rel_path, f"duplicate public Web package directory: {package_dir}")
        entries.add(entry)
        public_presets.add(preset)
        package_dirs.add(package_dir)
        if preset not in preset_names:
            fail(rel_path, f"public surface {entry} references unknown preset {preset}")
        if package_dir != f"pkg/{entry}":
            fail(rel_path, f"public surface {entry} pkg_dir_rel must be pkg/{entry}")
        if runtime_profile not in WEB_RUNTIME_PROFILES:
            fail(
                rel_path,
                f"public surface {entry} has unknown runtime profile {runtime_profile}",
            )

    return descriptor


def require_exact_keys(
    value: Any,
    expected: set[str],
    rel_path: str,
    label: str,
) -> None:
    if not isinstance(value, dict):
        fail(rel_path, f"{label} must be an object")
    actual = set(value)
    if actual != expected:
        fail(
            rel_path,
            f"{label} keys must be exactly: {', '.join(sorted(expected))}",
        )


def require_web_name(value: Any, rel_path: str, label: str) -> str:
    if not isinstance(value, str) or not re.fullmatch(r"[a-z0-9][a-z0-9-]*", value):
        fail(rel_path, f"{label} must be a lowercase kebab-case name")
    return value


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

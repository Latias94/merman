#!/usr/bin/env python3
"""Validate the stable workflow contract of the Mermaid alignment skill."""

from __future__ import annotations

import copy
import json
import re
import sys
import tempfile
from pathlib import Path

SKILL_HEADINGS = (
    "Read the Authority",
    "Establish the Reference Graph",
    "Materialize Sources Safely",
    "Inventory the Delta",
    "Admit Every Capability",
    "Decide Feature Boundaries",
    "Verify and Hand Off",
)

CHECKLIST_HEADINGS = (
    "Reference Graph",
    "Supply Chain and Lifecycle",
    "Delta Inventory",
    "Capability Admission",
    "Feature Decision",
    "Verification Evidence",
    "Forward Scenarios",
    "Handoff",
)

REQUIRED_SKILL_TEXT = (
    "verify-mermaid-reference",
    "verify-playground-example-catalog",
    "verify-web-diagram-catalog",
    "check-alignment",
    "verify --strict",
    "wasm-size-matrix",
    "references/admission-checklist.md",
    "Latest-compatible candidate",
    "Latest-stable delta",
    "Parser-only support is incomplete",
)

FORBIDDEN_COMMANDS = re.compile(
    r"^\s*(?:git\s+push|gh\s+pr\s+create|npm\s+publish|cargo\s+publish)\b",
    re.MULTILINE,
)

HARDCODED_RELEASE = re.compile(
    r"\bMermaid\s+v?\d+\.\d+(?:\.\d+)?\b|\bmermaid@\d+\.\d+(?:\.\d+)?\b",
    re.IGNORECASE,
)

RELEASE_DELTA_FIXTURE = Path("tests/fixtures/minimal-release-delta.json")

BUILT_IN_DIAGRAM_EVIDENCE = {
    "parser": "parser",
    "editor": "editor",
    "render": "render",
    "playground": "Playground",
}

FEATURE_EVIDENCE = (
    "dependency_graph",
    "targets",
    "licenses",
    "clean_build",
    "artifact_size",
    "package_surfaces",
)

REQUIRED_COMMAND_SEQUENCE = (
    "cargo run -p xtask -- verify-mermaid-reference --materialized",
    "cargo run -p xtask -- verify-playground-example-catalog",
    "cargo run -p xtask -- verify-web-diagram-catalog",
    "cargo run -p xtask -- check-alignment",
    "cargo run -p xtask -- verify --strict",
    "cargo run -p xtask -- wasm-size-matrix --budget-file "
    "docs/release/WASM_SIZE_BUDGETS.json",
)

SEMVER_MAJOR = re.compile(r"^v?(0|[1-9]\d*)\.")


def missing_headings(text: str, headings: tuple[str, ...]) -> list[str]:
    present = {
        match.group(1).strip()
        for match in re.finditer(r"^##\s+(.+?)\s*$", text, re.MULTILINE)
    }
    return [heading for heading in headings if heading not in present]


def main() -> int:
    if len(sys.argv) > 2:
        print("usage: validate_workflow.py [skill-root]", file=sys.stderr)
        return 2

    skill_root = (
        Path(sys.argv[1]).resolve()
        if len(sys.argv) == 2
        else Path(__file__).resolve().parents[1]
    )
    failures = validate(skill_root)
    if not failures:
        failures.extend(forward_test_required_sections(skill_root))
    if not failures:
        failures.extend(forward_test_release_delta(skill_root))
    return report(failures)


def validate(skill_root: Path) -> list[str]:
    skill_path = skill_root / "SKILL.md"
    checklist_path = skill_root / "references" / "admission-checklist.md"
    release_delta_path = skill_root / RELEASE_DELTA_FIXTURE
    failures: list[str] = []
    for path in (skill_path, checklist_path, release_delta_path):
        if not path.is_file():
            failures.append(f"missing required file: {path.relative_to(skill_root)}")

    if failures:
        return failures

    skill = skill_path.read_text(encoding="utf-8")
    checklist = checklist_path.read_text(encoding="utf-8")

    failures.extend(
        f"SKILL.md is missing workflow heading: {heading}"
        for heading in missing_headings(skill, SKILL_HEADINGS)
    )
    failures.extend(
        f"admission-checklist.md is missing heading: {heading}"
        for heading in missing_headings(checklist, CHECKLIST_HEADINGS)
    )
    failures.extend(
        f"SKILL.md is missing required contract text: {item}"
        for item in REQUIRED_SKILL_TEXT
        if item not in skill
    )

    combined = f"{skill}\n{checklist}"
    if "TODO" in combined:
        failures.append("skill resources contain unresolved TODO text")
    if HARDCODED_RELEASE.search(combined):
        failures.append("skill resources hardcode a Mermaid release")
    if FORBIDDEN_COMMANDS.search(combined):
        failures.append("skill resources contain an external delivery command")

    release_delta, load_failures = load_release_delta(release_delta_path)
    failures.extend(load_failures)
    if release_delta is not None:
        failures.extend(validate_release_delta(release_delta))

    return failures


def load_release_delta(path: Path) -> tuple[dict[str, object] | None, list[str]]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return None, [f"cannot read release delta fixture: {error}"]
    if not isinstance(value, dict):
        return None, ["release delta fixture root must be an object"]
    return value, []


def non_empty_text(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def semver_major(value: object) -> int | None:
    if not isinstance(value, str):
        return None
    match = SEMVER_MAJOR.match(value)
    return int(match.group(1)) if match else None


def validate_release_delta(release_delta: dict[str, object]) -> list[str]:
    failures: list[str] = []

    release = release_delta.get("release")
    if not isinstance(release, dict):
        failures.append("release delta is missing selected release identity")
    else:
        for field in ("selected_tag", "selected_commit"):
            if not non_empty_text(release.get(field)):
                failures.append(f"release delta is missing release field: {field}")

    capabilities = release_delta.get("capabilities")
    if not isinstance(capabilities, list) or not capabilities:
        failures.append("release delta has no capability inventory")
    else:
        for index, capability in enumerate(capabilities):
            if not isinstance(capability, dict):
                failures.append(f"capability at index {index} must be an object")
                continue
            capability_id = capability.get("id")
            label = capability_id if non_empty_text(capability_id) else f"index {index}"
            if not non_empty_text(capability.get("capability_owner")):
                failures.append(f"capability {label} is missing capability owner")
            if (
                capability.get("kind") != "built-in-diagram"
                or capability.get("change") != "added"
            ):
                continue
            evidence = capability.get("evidence")
            for field, display_name in BUILT_IN_DIAGRAM_EVIDENCE.items():
                value = evidence.get(field) if isinstance(evidence, dict) else None
                if not non_empty_text(value):
                    failures.append(
                        f"capability {label} is missing {display_name} evidence"
                    )

    companions = release_delta.get("companions")
    if not isinstance(companions, list):
        failures.append("release delta companion inventory must be a list")
    else:
        outside_major_count = 0
        for index, companion in enumerate(companions):
            if not isinstance(companion, dict):
                failures.append(f"companion at index {index} must be an object")
                continue
            if companion.get("range_relation") != "outside-range-major":
                continue
            outside_major_count += 1
            package = companion.get("package")
            label = package if non_empty_text(package) else f"index {index}"
            oracle_major = semver_major(companion.get("oracle_version"))
            candidate_major = semver_major(companion.get("candidate_version"))
            declared_range = companion.get("declared_range")
            declared_major = (
                semver_major(declared_range[1:])
                if isinstance(declared_range, str) and declared_range.startswith("^")
                else None
            )
            if (
                oracle_major is None
                or candidate_major is None
                or declared_major != oracle_major
                or candidate_major <= oracle_major
            ):
                failures.append(
                    f"companion {label} does not prove an outside-range major"
                )
            if companion.get("disposition") != "separately-scoped":
                failures.append(
                    f"companion {label} outside-range major must be separately-scoped"
                )
            if not non_empty_text(companion.get("capability_owner")):
                failures.append(f"companion {label} is missing capability owner")
        if outside_major_count == 0:
            failures.append(
                "release delta does not cover an outside-range companion major"
            )

    feature_decision = release_delta.get("feature_decision")
    if not isinstance(feature_decision, dict):
        failures.append("release delta is missing a feature decision")
    else:
        if feature_decision.get("outcome") not in {"split", "no-split"}:
            failures.append("feature decision outcome must be split or no-split")
        if not non_empty_text(feature_decision.get("reason")):
            failures.append("feature decision is missing a reason")
        feature_evidence = feature_decision.get("evidence")
        for field in FEATURE_EVIDENCE:
            value = (
                feature_evidence.get(field)
                if isinstance(feature_evidence, dict)
                else None
            )
            if not non_empty_text(value):
                failures.append(f"feature decision is missing evidence: {field}")

    commands = release_delta.get("command_sequence")
    if not isinstance(commands, list) or not all(
        non_empty_text(command) for command in commands
    ):
        failures.append("release delta command sequence must be a list of commands")
    else:
        next_index = 0
        for required in REQUIRED_COMMAND_SEQUENCE:
            try:
                next_index = commands.index(required, next_index) + 1
            except ValueError:
                failures.append(
                    f"release delta command sequence is missing or misorders: {required}"
                )
                break

    return failures


def forward_test_required_sections(skill_root: Path) -> list[str]:
    source_files = {
        "SKILL.md": (skill_root / "SKILL.md").read_text(encoding="utf-8"),
        "references/admission-checklist.md": (
            skill_root / "references" / "admission-checklist.md"
        ).read_text(encoding="utf-8"),
        str(RELEASE_DELTA_FIXTURE): (skill_root / RELEASE_DELTA_FIXTURE).read_text(
            encoding="utf-8"
        ),
    }
    mutations = [
        ("SKILL.md", heading, f"SKILL.md is missing workflow heading: {heading}")
        for heading in SKILL_HEADINGS
    ] + [
        (
            "references/admission-checklist.md",
            heading,
            f"admission-checklist.md is missing heading: {heading}",
        )
        for heading in CHECKLIST_HEADINGS
    ]

    failures: list[str] = []
    for relative_path, heading, expected_failure in mutations:
        marker = f"## {heading}"
        mutated = source_files[relative_path].replace(
            marker, f"## Removed {heading}", 1
        )
        if mutated == source_files[relative_path]:
            failures.append(f"forward test could not remove heading: {heading}")
            continue

        with tempfile.TemporaryDirectory(prefix="align-mermaid-release-") as temp_dir:
            fixture_root = Path(temp_dir)
            for fixture_path, content in source_files.items():
                destination = fixture_root / fixture_path
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(
                    mutated if fixture_path == relative_path else content,
                    encoding="utf-8",
                )
            observed = validate(fixture_root)
        if expected_failure not in observed:
            failures.append(f"forward test accepted missing heading: {heading}")

    return failures


def forward_test_release_delta(skill_root: Path) -> list[str]:
    release_delta, failures = load_release_delta(skill_root / RELEASE_DELTA_FIXTURE)
    if release_delta is None:
        return failures

    capabilities = release_delta.get("capabilities")
    built_in_index = (
        next(
            (
                index
                for index, capability in enumerate(capabilities)
                if isinstance(capability, dict)
                and capability.get("kind") == "built-in-diagram"
                and capability.get("change") == "added"
            ),
            None,
        )
        if isinstance(capabilities, list)
        else None
    )
    if built_in_index is None:
        return ["forward test fixture has no added built-in diagram"]

    capability = capabilities[built_in_index]
    capability_id = capability.get("id")
    label = (
        capability_id if non_empty_text(capability_id) else f"index {built_in_index}"
    )

    for field, display_name in BUILT_IN_DIAGRAM_EVIDENCE.items():
        mutated = copy.deepcopy(release_delta)
        mutated_capability = mutated["capabilities"][built_in_index]
        mutated_capability["evidence"].pop(field, None)
        expected = f"capability {label} is missing {display_name} evidence"
        if expected not in validate_release_delta(mutated):
            failures.append(
                f"forward test accepted built-in diagram without {display_name} evidence"
            )

    mutated = copy.deepcopy(release_delta)
    mutated["capabilities"][built_in_index].pop("capability_owner", None)
    expected = f"capability {label} is missing capability owner"
    if expected not in validate_release_delta(mutated):
        failures.append("forward test accepted a capability without an owner")

    companions = release_delta.get("companions")
    outside_index = (
        next(
            (
                index
                for index, companion in enumerate(companions)
                if isinstance(companion, dict)
                and companion.get("range_relation") == "outside-range-major"
            ),
            None,
        )
        if isinstance(companions, list)
        else None
    )
    if outside_index is None:
        failures.append("forward test fixture has no outside-range companion major")
    else:
        mutated = copy.deepcopy(release_delta)
        outside = mutated["companions"][outside_index]
        outside["disposition"] = "selected"
        package = outside.get("package")
        label = package if non_empty_text(package) else f"index {outside_index}"
        expected = f"companion {label} outside-range major must be separately-scoped"
        if expected not in validate_release_delta(mutated):
            failures.append(
                "forward test accepted an outside-range companion major as selected"
            )

        mutated = copy.deepcopy(release_delta)
        outside = mutated["companions"][outside_index]
        outside["candidate_version"] = outside["oracle_version"]
        expected = f"companion {label} does not prove an outside-range major"
        if expected not in validate_release_delta(mutated):
            failures.append(
                "forward test trusted an outside-range label without a newer major"
            )

    mutated = copy.deepcopy(release_delta)
    mutated["feature_decision"].pop("reason", None)
    if "feature decision is missing a reason" not in validate_release_delta(mutated):
        failures.append("forward test accepted an unreasoned feature decision")

    for field in FEATURE_EVIDENCE:
        mutated = copy.deepcopy(release_delta)
        mutated["feature_decision"]["evidence"].pop(field, None)
        expected = f"feature decision is missing evidence: {field}"
        if expected not in validate_release_delta(mutated):
            failures.append(
                f"forward test accepted a feature decision without evidence: {field}"
            )

    commands = release_delta.get("command_sequence")
    if not isinstance(commands, list):
        failures.append("forward test fixture has no command sequence")
    else:
        for command in REQUIRED_COMMAND_SEQUENCE:
            mutated = copy.deepcopy(release_delta)
            mutated["command_sequence"].remove(command)
            expected = (
                f"release delta command sequence is missing or misorders: {command}"
            )
            if expected not in validate_release_delta(mutated):
                failures.append(
                    f"forward test accepted a command sequence without: {command}"
                )

    return failures


def report(failures: list[str]) -> int:
    if failures:
        print("align-mermaid-release workflow validation failed:", file=sys.stderr)
        for failure in failures:
            print(f"- {failure}", file=sys.stderr)
        return 1

    print("align-mermaid-release workflow validation passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

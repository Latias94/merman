#!/usr/bin/env python3
"""Validate the stable workflow contract of the Mermaid alignment skill."""

from __future__ import annotations

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
    return report(failures)


def validate(skill_root: Path) -> list[str]:
    skill_path = skill_root / "SKILL.md"
    checklist_path = skill_root / "references" / "admission-checklist.md"
    failures: list[str] = []
    for path in (skill_path, checklist_path):
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

    return failures


def forward_test_required_sections(skill_root: Path) -> list[str]:
    source_files = {
        "SKILL.md": (skill_root / "SKILL.md").read_text(encoding="utf-8"),
        "references/admission-checklist.md": (
            skill_root / "references" / "admission-checklist.md"
        ).read_text(encoding="utf-8"),
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
        mutated = source_files[relative_path].replace(marker, f"## Removed {heading}", 1)
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

#!/usr/bin/env python3
"""Validate the stable workflow contract of the Mermaid alignment skill."""

from __future__ import annotations

import re
import sys
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
    "cargo run -p xtask -- verify-mermaid-reference --materialized",
    "cargo run -p xtask -- verify-playground-example-catalog",
    "cargo run -p xtask -- verify-web-diagram-catalog",
    "cargo run -p xtask -- gen-lalrpop-parsers",
    "cargo run -p xtask -- verify-lalrpop-parsers",
    "cargo run -p xtask -- check-alignment",
    "cargo run -p xtask -- verify --strict",
    "cargo run -p xtask -- wasm-size-matrix --budget-file "
    "docs/release/WASM_SIZE_BUDGETS.json",
    "cargo nextest",
    "cargo fmt --all -- --check",
    "cargo clippy --workspace --all-targets --all-features -- -D warnings",
    "quick_validate.py",
    "Latest-compatible candidate",
    "Latest-stable delta",
    "Parser-only support is incomplete",
    "Treat push, PR creation, package publication, and release as separate authority.",
)

REQUIRED_SKILL_REFERENCES = (
    "docs/release/MERMAID_UPGRADE_PLAYBOOK.md",
    "tools/upstreams/REPOS.lock.json",
    "tools/upstreams/README.md",
    "docs/FEATURES.md",
    "docs/release/PACKAGE_SURFACES.md",
    "docs/release/WASM_SIZE_BUDGETS.json",
    "references/admission-checklist.md",
)

REQUIRED_CHECKLIST_TEXT = (
    "Parser-only support does not close admission.",
    "Push, PR, publication, and release remain outside the handoff unless separately requested.",
)

FORBIDDEN_COMMANDS = re.compile(
    r"""
    ^\s*(?:\$\s*)?(?:
        git\s+(?:push|tag)\b
        |gh\s+(?:pr\s+create|release(?:\s+\w+)?|workflow\s+run)\b
        |(?:npm|pnpm)\s+publish\b
        |yarn\s+npm\s+publish\b
        |cargo\s+publish\b
        |dart\s+pub\s+publish\b
        |twine\s+upload\b
    )
    """,
    re.MULTILINE | re.VERBOSE,
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
    return report(validate(skill_root))


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
    failures.extend(
        f"SKILL.md is missing required reference: {reference}"
        for reference in REQUIRED_SKILL_REFERENCES
        if reference not in skill
    )
    failures.extend(
        f"admission-checklist.md is missing required contract text: {item}"
        for item in REQUIRED_CHECKLIST_TEXT
        if item not in checklist
    )

    combined = f"{skill}\n{checklist}"
    if "TODO" in combined:
        failures.append("skill resources contain unresolved TODO text")
    if HARDCODED_RELEASE.search(combined):
        failures.append("skill resources hardcode a Mermaid release")
    if FORBIDDEN_COMMANDS.search(combined):
        failures.append("skill resources contain an external delivery command")

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

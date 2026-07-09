---
type: "Work Registration"
title: "Mermaid 11.16 parity alignment"
description: "Registration for Mermaid 11.16 parity alignment."
timestamp: 2026-07-09T11:13:56Z
status: "active"
last_seen: 2026-07-09T11:13:56Z
producer_id: "codex-root"
related_plan: "docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
git_commit: "4d30083d1014f5924ed5b410c2eefcd15d0d9ddc"
latest_link: "Previous baseline commit: 41646dfd43ac83f001b03c70605feb036afae46d"
---

# Scope

Align Merman from mermaid@11.15.0 to mermaid@11.16.0 as a headless, source-backed Mermaid implementation. Scope includes baseline metadata, configuration and theme semantics, existing diagram family deltas, new 11.16 family admission, fixture/baseline refresh, SVG DOM parity, and cleanup of obsolete 11.15-only code.

# Current Claim

Implementation-ready plan exists and is the execution authority. Plan review corrected Mermaid 11.16 id/header mapping for swimlane, cynefin, and railroad variants; the upstream tag commit is 5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c. Execution allows breaking changes, broad refactors, staged new-family admission only with evidence, and deletion of obsolete 11.15-only code.

# Latest Links

- Plan: `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
- Mermaid 11.16 tag commit: `5e3c88ea6d937a89078a5e8f1b2a6fd0ea391a5c`
- Previous baseline commit: 41646dfd43ac83f001b03c70605feb036afae46d

# Handoff

Start with the plan's Goal Capsule, Acceptance Examples, Key Technical Decisions, and Implementation Units. Track execution outside the plan file. Before Rust code edits, load repo Rust guidance, use nextest/fmt, protect unrelated user changes, and keep verification focused per unit with full parity gates before completion.

# Citations

- `repo-ref/mermaid` tag `mermaid@11.16.0`
- `repo-ref/mermaid` tag `mermaid@11.15.0`
- `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`

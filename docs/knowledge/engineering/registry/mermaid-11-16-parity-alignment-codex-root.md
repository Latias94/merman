---
type: "Work Registration"
title: "Mermaid 11.16 parity alignment"
description: "Registration for Mermaid 11.16 parity alignment."
timestamp: 2026-07-09T11:02:34Z
status: "active"
last_seen: 2026-07-09T11:02:34Z
producer_id: "codex-root"
related_plan: "docs\\plans\\2026-07-09-002-refactor-mermaid-11-16-parity-plan.md"
git_branch: "feat/mermaid-11-16-parity"
---

# Scope

Align Merman from `mermaid@11.15.0` to `mermaid@11.16.0` as a headless,
source-backed Mermaid implementation. Scope includes baseline metadata,
configuration and theme semantics, existing diagram family deltas, new 11.16
family admission, fixture/baseline refresh, SVG DOM parity, and cleanup of
obsolete 11.15-only code.


# Current Claim

An implementation-ready plan exists and should be treated as the execution
authority. The active branch is `feat/mermaid-11-16-parity`; the plan allows
breaking changes, broad refactors, staged new-family admission, and deletion of
obsolete code when source-backed by the 11.16 Mermaid source.


# Latest Links

- Plan: `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`
- Upstream source: `repo-ref/mermaid` at `mermaid@11.16.0`
- Previous baseline: `mermaid@11.15.0`

# Handoff

Start with the plan's Goal Capsule, Requirements, Key Technical Decisions, and
Implementation Units. Track execution outside the plan file. Before Rust code
edits, apply the repository Rust guidance and keep verification focused per
unit, with full parity gates before completion.


# Citations

- `repo-ref/mermaid` tag `mermaid@11.16.0`
- `docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md`

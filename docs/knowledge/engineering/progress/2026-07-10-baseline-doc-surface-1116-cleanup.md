---
type: Work Progress
title: Mermaid 11.16 baseline documentation surface cleanup
timestamp: 2026-07-10T01:08:10+08:00
status: active
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
git_commit: fb54748a247f
tags: mermaid-11-16,baseline,ce-work
---

# Summary

The current-facing baseline documentation and a few active code comments now advertise Mermaid
`@11.16.0` instead of stale `@11.15.0` claims.

# Changed

- Updated root-facing baseline surfaces: `CONTEXT.md`, `README.md`, `THIRD_PARTY_NOTICES.md`,
  `docs/adr/0001-upstream-baseline.md`, and `crates/merman/src/lib.rs`.
- Updated active alignment coverage docs from Mermaid `@11.15.0` headings/scopes to Mermaid
  `@11.16.0`.
- Kept historical references explicit where they are still facts: published benchmark numbers,
  Phase 2 historical admission, old generated override filename suffixes, and Mermaid issue #7954's
  11.15-vs-11.16 regression evidence.
- Renamed the Sequence SVG extra marker defs constant from a 11.15-specific name to a pinned-baseline
  name.
- Updated xtask tests that mocked the pinned Mermaid baseline label.

# Boundary

Do not rewrite historical `docs/workstreams/**`, old quality reports, or old plan evidence only to
remove `11.15.0` text. Those files are archives. Current-facing docs should either say `@11.16.0` or
mark old version references as historical/legacy provenance.

# Next Action

Commit this cleanup after final verification, then perform the plan-level DoD audit before marking
the Mermaid 11.16 parity goal complete.

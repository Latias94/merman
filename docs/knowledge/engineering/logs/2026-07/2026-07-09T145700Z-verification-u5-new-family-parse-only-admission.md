---
type: Verification Evidence
title: "U5 new-family staged admission"
timestamp: 2026-07-09T14:57:00Z
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
tags:
  - mermaid-11-16
  - u5
  - admission
  - fixtures
---

# Summary

The 11.16 new-family slices are now represented in the admission inventory and fixture corpus
instead of only in unit tests.

# Evidence

- `swimlane` has parse-only semantic fixture coverage under `fixtures/swimlane`.
- `cynefin` has semantic and layout fixture coverage under `fixtures/cynefin` and is admitted as
  `CompatibilityOnly` pending upstream SVG baselines.
- The four railroad dialect ids have parse-only semantic fixture coverage under
  `fixtures/railroad`, `fixtures/railroadEbnf`, `fixtures/railroadAbnf`, and
  `fixtures/railroadPeg`.
- Admission remains intentionally staged for the families without source-backed renderers and for
  primary SVG matrix coverage until upstream SVG baselines and compare commands are admitted.

# Verification

- Generated semantic goldens with `cargo run -p xtask -- update-snapshots --diagram <family>` for
  all six new-family directories.
- Generated the Cynefin layout golden with
  `cargo run -p xtask -- update-layout-snapshots --diagram cynefin`.

# Next Action

Proceed into Railroad renderer planning or the source-backed Swimlane layout port after committing
the staged U5 admission slice.

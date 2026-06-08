# Post Alpha.2 Fearless Refactor

Status: Complete
Last updated: 2026-06-08

## Goal

Clean the release-facing architecture after `0.7.0-alpha.2` so the next alpha does not preserve shallow implementation-era seams as compatibility promises.

`compound-engineering:ce-plan` is recorded in `docs/plans/2026-06-08-001-refactor-post-alpha2-fearless-refactor-plan.md`. This document remains the local closeout brief for the completed workstream.

## Priority Order

1. Binding Render Request Module: deepen `merman-bindings-core` so options JSON maps to one render request plan shared by one-shot and cached engine entry points.
2. Diagram Family Facts Module: continue consolidating detector order, fast detect, parser adapters, render adapters, fallback policy, and metadata projections.
3. Render-Side Presentation Theme View: continue ADR-0068 migrations for high-duplication raw `themeVariables` readers.
4. Public Headless Operation Interface Cleanup: make canonical operation paths obvious and keep lower-level parse/layout surfaces as expert/debug paths.
5. Admission Inventory Module: keep fixture admission, coverage, root status, skip/defer reasons, and report projections together.
6. Xtask Parity Harness Module: reduce compare/import/audit harness duplication and make DOM policy reporting more explicit.

## Constraints

- Keep public binding ABI and options JSON stable unless a new ADR records the contract change.
- Prefer deleting pass-through helpers and duplicate policy only after a deeper Module owns the behavior.
- Keep Mermaid parity source-backed; do not hide browser-dependent residuals behind broad comparator normalization.
- Use focused verification first, then widen when a change touches shared contracts.

## Success Criteria

- One-shot and cached binding render paths use the same request Module and classification policy.
- Any deleted code is proven redundant by call-site search and focused tests.
- Each completed slice leaves tests closer to the Module Interface rather than private implementation details.
- Workstream docs and evidence describe remaining follow-ons without reopening closed historical lanes.

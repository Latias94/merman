---
type: Verification Evidence
title: Railroad 11.16 compatibility renderer verification
timestamp: 2026-07-10T00:45:19+08:00
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,railroad,verification
---

# Verification

Commands run after implementing the Railroad compatibility renderer:

- `cargo check -p merman-render --tests` - passed.
- `cargo nextest run -p merman-core railroad diagram_family_capabilities_follow_detector_and_parser_fact_projection supported_diagram_metadata_is_backed_by_typed_render_projection fixtures_match_golden_snapshots --no-fail-fast` - passed.
- `cargo nextest run -p merman-render railroad render_model_dispatch_renders_railroad_svg fixtures_match_layout_golden_snapshots_when_present --no-fail-fast` - passed.
- `cargo nextest run -p merman-bindings-core diagram_family_capabilities_expose_detector_parser_and_render_surface --no-fail-fast` - passed.
- `cargo nextest run -p xtask admission --no-fail-fast` - passed.
- `cargo run -p xtask -- check-alignment` - passed after adding `RAILROAD_UPSTREAM_TEST_COVERAGE.md`.
- `cargo fmt --check` - passed.
- `git diff --check` - passed.

# Notes

The first layout gate attempt used an overly broad nextest filter and timed out after 5 minutes with
no assertion failure output. The focused layout harness rerun passed.

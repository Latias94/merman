---
type: Verification Evidence
title: Mermaid 11.16 baseline documentation surface cleanup verification
timestamp: 2026-07-10T01:08:10+08:00
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
git_commit: fb54748a247f
tags: mermaid-11-16,baseline,verification
---

# Verification

Commands run after the baseline surface cleanup:

- `cargo fmt --check` - passed.
- `cargo run -p xtask -- check-alignment` - passed.
- `cargo nextest run -p xtask pinned_mermaid_baseline_label_reads_lockfile_ref render_path_probe_markdown_summarizes_facts_and_stages --no-fail-fast` - passed.
- `cargo check -p merman-render --tests` - passed.
- `cargo nextest run -p merman-render flowchart_node_shape_dimensions_follow_mermaid_rules --no-fail-fast` - passed.
- `cargo check -p merman --tests` - passed.
- `cargo nextest run -p xtask admission --no-fail-fast` - passed.
- `git diff --check` - passed.

# Search Audit

Targeted search over root docs, active alignment docs, ADRs, rendering docs, tools, and crates now
leaves only explicit historical or legacy `11.15.0` references: benchmark notes, historical
admission records, generated override suffixes, Mermaid issue #7954 regression evidence, and
archived workstream/quality/plan/memory material.

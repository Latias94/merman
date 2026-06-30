---
type: Work Progress
status: completed
related_plan: ../../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
verified_by: cargo test; cargo fmt; git diff --check
---

# Gantt Parser-Backed Entity Links

- Date: 2026-06-27
- Branch: `feat/diagnostics-analysis-contract`
- Goal: keep pushing `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`
  toward mature Mermaid LSP coverage with fewer transport-local heuristics.

## What Changed

- `crates/merman-core/src/diagrams/gantt/parse.rs` now treats task ids, dependency refs, and click
  target ids as the same semantic entity kind so rename/reference queries connect them.
- Those same Gantt id spans now emit parser-backed `NodeIdentifier` expected syntax, giving the
  editor layer source-backed cursor context for dependency and click target positions.

## Why It Matters

- Renaming a Gantt task now updates its dependency references instead of only touching the task
  definition.
- Gantt dependency and click target positions are now parser-backed entity positions instead of
  ad-hoc payload-ish spans.
- This closes a real semantic split that would have left rename/reference behavior incomplete even
  though the parser already knew the right spans.

## Verification

- `cargo test -p merman-core --lib gantt -- --nocapture`
- `cargo test -p merman-lsp --lib context::tests::context_classifies_header_operator_and_directive_prefixes -- --nocapture`
- `cargo test -p merman-lsp --lib structure::tests::gantt_rename_and_references_track_dependency_refs -- --nocapture`
- `cargo test -p merman-lsp --test completion -- --nocapture`
- `cargo test -p merman-analysis --lib editor -- --nocapture`
- `cargo fmt --all`
- `git diff --check`

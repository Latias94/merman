---
type: Work Progress
status: completed
related_plan: ../../../plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
verified_by: cargo test; cargo fmt; git diff --check
---

# Mindmap Parser-Backed Node Payload Completion

- Date: 2026-06-27
- Branch: `feat/diagnostics-analysis-contract`
- Goal: keep pushing `docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md`
  toward mature Mermaid LSP coverage with fewer transport-local heuristics.

## What Changed

- `crates/merman-core/src/diagrams/mindmap/parse.rs` now records parser-backed expected syntax for
  mindmap node id spans and node payload spans.
- `crates/merman-core/src/diagrams/mindmap/utils.rs` now returns spanned node-spec data so the
  parser can distinguish id regions from explicit payload regions.
- `crates/merman-lsp/src/context.rs` now treats parser-known `NodeIdentifier` and `Payload`
  positions as authoritative before falling back to shape/directive/node heuristics.

## Why It Matters

- Mindmap explicit labels now suppress generic node-id completion inside payload text.
- Flowchart shape keywords no longer leak into mindmap node label positions just because the line
  contains `[` or `(`.
- The next parser-backed completion slices can keep tightening the same contract family by family
  instead of inventing more LSP-local guesses.

## Verification

- `cargo test -p merman-core --lib mindmap -- --nocapture`
- `cargo test -p merman-lsp --test completion -- --nocapture`
- `cargo test -p merman-analysis --lib editor -- --nocapture`
- `cargo fmt --all`
- `cargo fmt --all --check`
- `git diff --check`

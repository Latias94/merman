---
type: "Session Handoff"
title: "Merman parser-backed facts recovery continuation - venn complete"
description: "Continuation note after recovering the parser-backed editor facts work and finishing the venn family."
timestamp: 2026-06-28T00:00:00Z
tags: ["merman", "lsp", "semantic-facts", "session-handoff", "venn"]
source_session: "019ef370-dae4-7382-b0df-bbdb9ebe2d1b"
git_branch: "feat/diagnostics-analysis-contract"
---

# Summary

Recovered the ongoing parser-backed editor facts work, then completed the `venn` family so it now uses parser-backed semantic facts.

# Verified State

- `parse_venn_editor_facts` is implemented in `crates/merman-core/src/diagrams/venn.rs`.
- `parse_pipeline.rs` now dispatches `"venn"` to that parser-backed editor facts entry point.
- The focused regression `parse_venn_editor_facts_expose_parser_backed_spans` passes.
- The existing `venn` render-model tests still pass.
- `cargo fmt --all --check` passes.

# Open Threads

- More diagram families still rely on heuristic editor facts paths.
- The long-term roadmap still includes diagnostics/lint, completion, hover, symbols, definition, references, rename, code actions, semantic tokens, and binding surfaces.

# Next Action

Continue with the next family that still needs parser-backed semantic facts, then add a focused regression test and rerun the narrow verification commands.

# Citations

- `crates/merman-core/src/diagrams/venn.rs`
- `crates/merman-core/src/parse_pipeline.rs`
- `cargo test -p merman-core --lib venn -- --nocapture`
- `cargo fmt --all --check`

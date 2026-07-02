---
type: "Session Handoff"
title: "Merman parser-backed facts recovery continuation"
description: "Session handoff after recovering the ongoing parser-backed editor facts work and verifying requirement facts routing."
timestamp: 2026-06-28T00:00:00Z
tags: ["merman", "lsp", "semantic-facts", "session-handoff", "requirement"]
source_session: "019ef370-dae4-7382-b0df-bbdb9ebe2d1b"
git_branch: "feat/diagnostics-analysis-contract"
---

# Summary

Recovered the current parser-backed editor facts work and verified the `requirement` family now routes through `parse_requirement_editor_facts`.

# Verified State

- `parse_requirement_editor_facts` is implemented in `crates/merman-core/src/diagrams/requirement.rs`.
- `parse_pipeline.rs` now dispatches `"requirement"` to that parser-backed editor facts entry point.
- The requirement regression `parse_requirement_editor_facts_expose_parser_backed_spans` passes.
- The existing requirement render-model test still passes.
- `cargo fmt --all --check` passes.

# Open Threads

- More diagram families may still rely on heuristic editor facts paths.
- The long-term roadmap still includes diagnostics/lint, completion, hover, symbols, definition, references, rename, code actions, semantic tokens, and binding surfaces.

# Next Action

Continue with the next family that still needs parser-backed semantic facts, then add a focused regression test and rerun the narrow verification commands.

# Citations

- `crates/merman-core/src/diagrams/requirement.rs`
- `crates/merman-core/src/parse_pipeline.rs`
- `crates/merman-core/src/tests/misc.rs`
- `cargo test -p merman-core --lib parse_requirement_editor_facts_expose_parser_backed_spans -- --nocapture`
- `cargo test -p merman-core --lib requirement_full_requirement_definition_is_parsed -- --nocapture`
- `cargo fmt --all --check`

---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Summary

Flowchart `shapeData` values now drive parser-backed shape completion instead of relying only on
line-prefix guessing.

# Details

- `merman-core` now emits `EditorExpectedSyntaxKind::ShapeValue` for flowchart `@{ shape: ... }`
  values on both the complete AST path and the recovered token-scan path.
- `merman-analysis` maps that core syntax kind into `FenceExpectedSyntaxKind::Shape` so parser
  facts suppress generic node completions and keep shape completion active.
- `merman-lsp` now uses the parser-backed shape span to compute the edit range for multiline
  shape values, so completion can replace the value text without depending on the old
  `@{ shape:` prefix heuristic.

# Verification

- `cargo test -p merman-core --lib flowchart -- --nocapture`
- `cargo test -p merman-analysis --lib editor -- --nocapture`
- `cargo test -p merman-lsp --lib context::tests::context_classifies_header_operator_and_directive_prefixes -- --nocapture`
- `cargo test -p merman-lsp --test completion shape -- --nocapture`
- `cargo fmt --all --check`
- `git diff --check`

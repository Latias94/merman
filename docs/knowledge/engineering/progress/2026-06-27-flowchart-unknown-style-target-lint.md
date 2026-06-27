---
type: Work Progress
status: active
related_plan: docs/plans/2026-06-24-003-refactor-mature-mermaid-lsp-roadmap-plan.md
git_branch: feat/diagnostics-analysis-contract
---

# Flowchart Unknown Style Target Lint

- Date: 2026-06-27
- Goal: broaden U4 lint coverage with a source-backed Mermaid compatibility warning instead of a
  Merman-only style preference.

## What Changed

- `merman-core` now emits `merman.semantic.flowchart.unknown_style_target` warning facts when a
  flowchart `style` directive targets a node that was not otherwise defined.
- The warning mirrors Mermaid's FlowDB behavior: Mermaid logs that this may indicate a typo and
  still auto-creates the node.
- `merman-analysis` registers the rule as a core Mermaid-compatibility semantic warning with
  stable metadata, evidence, severity, and configurability.

## Why It Matters

- This is real lint coverage grounded in upstream Mermaid behavior, not a Merman-authored
  formatting or style standard.
- The warning is produced from the flowchart semantic build path, so it avoids adding another
  raw-text heuristic to LSP or analysis.
- It reinforces the parser strategy recorded in the mature LSP plan: use family-local parser and
  semantic facts, and only consider parser-generator rewrites when a specific family needs them.

## Verification

- `cargo test -p merman-core parse_diagram_flowchart_style_statement_applies_vertex_styles -- --nocapture`
- `cargo test -p merman-core parse_diagram_flowchart_without_direction_preserves_source_and_warns -- --nocapture`
- `cargo test -p merman-analysis semantic_warning_facts_map_flowchart_unknown_style_target_rule_id -- --nocapture`
- `cargo test -p merman-analysis rule_descriptors_expose_stable_rule_metadata -- --nocapture`
- `cargo fmt --all`

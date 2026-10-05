---
type: "Work Progress"
title: "Mermaid 12 recovered renderer convergence and remaining admission gates"
description: "Verified recovery work without claiming incomplete family, residual, WASM or version admission."
timestamp: 2026-09-22T11:19:02Z
record_id: "e43ab81190b642cfb7e9b73a4659e485"
producer_id: "codex-mermaid12-recovery"
run_id: "session-01a0c895"
source_session: "01a0c895-fc2a-72d2-bae5-5286866a9537"
related_plan: "docs/plans/2026-09-20-1250-refactor-mermaid-12-alignment-plan.md"
git_branch: "refactor/mermaid-12-alignment"
git_commit: "b70aa1213"
---

# Scope and authority

Continue the complete Mermaid 12 plan toward v0.8.0-alpha.7. The maintainer confirmed the
existing scope and authorized subagents and local commits. No publishing, pushing, PR or merge.
Session 01a0c895-fc2a-72d2-bae5-5286866a9537 owns continuation; recovered session
01a0bd10-50cb-7453-b696-280a38ed10bc and its old agent handles are historical evidence only.

# Verified work

- `1783f71b8`: Requirement retains prototype-named nodes, edges and work accounting. All 34
  Requirement tests passed with layout-elk. The prototype fixture passed strict parity DOM comparison.
- `b70aa1213`: ADR-0089 records the default-ELK product boundary and supersedes ADR-0085/0088.
- Workspace cargo check passed. Subsequent Class/ER/Flowchart changes passed 65 targeted tests:
  `cargo nextest run -p merman-render --features layout-elk --test flowchart_mermaid12_styles_test
  --test flowchart_container_palette_test --test flowchart_mermaid12_dom_test
  --test class_mermaid12_dom_test --test er_mermaid12_markers_test --test class_svg_test
  --test er_svg_test --no-fail-fast` (`target/recovery-node-families-tests.log`).
- Those uncommitted fixes cover Class theme font size and rounded ELK paths, ER rounded paths,
  fill/masks and source-backed Neo stylesheet/palette behavior, and Flowchart shared Neo CSS.
- All 37 upstream SVG manifests now refer to authentic Mermaid 12 output (3716 SVGs).
  Prior current-state text about unfinished golden generation is stale.
- License verification and all 382 legal-material projections passed.

# Open admission gates

- Semantic label catalog migration is incomplete. C4, Requirement, State and Flowchart candidates
  were reviewed; Class still lacks shared terminal straightening. ER stylesheet now matches and
  fresh geometry candidates are in `target/recovery-er-label-candidates-3.log`.
- Class reads/caches extra terminal bends are a real implementation omission. Source-backed
  straightening accepts both changes without increasing crossings. Do not admit them as text residuals.
- Sequence special participants still need Mermaid 12 Neo actor-band geometry, palette/filter and
  footer-order convergence. Flowchart and State strict reports still contain routing differences.
- Browser text layout catalog still has 96 Mermaid 11 receipts. Migrate only after reviewing actual
  differences. Comparisons without `--check-dom` do not establish DOM parity.
- Fresh analysis WASM size: raw 3552597, stripped 3552374 (both pass 3600000), gzip 1415591
  (limit 1375000), Brotli 1090288 (limit 1050000). Older four-failure size logs are superseded.
  ASCII raw 5113745 passes 5175000; compressed measures still need checking.
- Full/render Web WASM are stale after renderer edits. Final rebuild, freshness checks, Playground
  build/browser smoke and affected integrated gates remain. Versions and changelog remain alpha.6
  until final alpha.7 integration.

# Continuation

Current agents own Sequence convergence, shared terminal straightening, and exact label residual
review. Coordinate shared renderer files before editing. Run Cargo serially and reuse target.
Preserve all inherited working-tree edits; stage focused reviewed hunks, never restore unrelated work.
The plan and goal are still active. Passing focused tests is not completion of U1-U15.

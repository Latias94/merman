---
type: Work Progress
title: Railroad 11.16 compatibility renderer
timestamp: 2026-07-10T00:45:19+08:00
status: active
related_plan: docs/plans/2026-07-09-002-refactor-mermaid-11-16-parity-plan.md
git_branch: feat/mermaid-11-16-parity
tags: mermaid-11-16,railroad,ce-work
---

# Summary

Railroad moved from parser-only evidence to a compatibility renderer slice for Mermaid `@11.16.0`.

# Implemented

- Added typed render parser projection for `railroad`, `railroadEbnf`, `railroadAbnf`, and
  `railroadPeg`.
- Added shared `RailroadDiagramRenderModel` based on the existing SourceSpan-preserving parser AST.
- Added Rust layout for the upstream recursive railroad renderer shape: terminals, nonterminals,
  specials, sequences, choices, optionals, repetitions, rule names, markers, and connector paths.
- Added SVG parity renderer with `railroad-diagram`, rule groups, element classes, connector paths,
  theme/config styling, and accessibility title/description output.
- Added layout goldens for all four railroad fixture directories.
- Updated core registry tests, bindings metadata tests, xtask admission inventory, and alignment docs
  from parser-only to `CompatibilityOnly`.

# Important Boundaries

- Keep `swimlane` parse-only until a real source-backed swimlane layout port exists. Do not map
  swimlane to ordinary Flowchart rendering just to get an SVG.
- Mermaid issue https://github.com/mermaid-js/mermaid/issues/7954 is an upstream 11.16.0 Flowchart
  subgraph-arrow regression. Treat affected fixtures as upstream-known regression evidence, not as
  a reason to restore 11.15 behavior or broaden comparator normalization.
- The upstream railroad renderer does not consume `compactMode` or `showMarkers` in drawing and does
  not draw repetition separator/max metadata. Local compatibility output follows that source behavior.

# Changed Areas

- Core: `crates/merman-core/src/diagrams/railroad.rs`, `diagram/mod.rs`, `family.rs`, registry and
  railroad tests.
- Render: `crates/merman-render/src/railroad.rs`, `src/svg/parity/railroad.rs`, layout model, SVG
  dispatch, and render smoke tests.
- Fixtures: `fixtures/railroad*/**/*.layout.golden.json`.
- Admission/docs: `crates/xtask/src/cmd/admission.rs`, `docs/alignment/RAILROAD_MINIMUM.md`,
  `docs/alignment/RAILROAD_UPSTREAM_TEST_COVERAGE.md`, `STATUS.md`, and the unsupported-family
  rubric.

# Next Action

Commit this railroad slice after final diff review. Then continue U5 by leaving swimlane explicitly
parse-only unless implementing the upstream swimlane layout utilities in a separate source-backed slice.

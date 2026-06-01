# Flowchart 11.15 SVG Convergence - Milestones

Status: Active
Last updated: 2026-06-01

## M0 - Scope And Evidence Freeze

Exit criteria:

- Fresh Mermaid 11.15 Flowchart target evidence is recorded.
- The child workstream documents why Flowchart is not a one-fixture MathML cleanup.
- First executable renderer slice is chosen.

Primary evidence:

- `docs/workstreams/flowchart-11-15-svg-convergence/DESIGN.md`
- `docs/workstreams/flowchart-11-15-svg-convergence/EVIDENCE_AND_GATES.md`

## M1 - DOM Envelope And Identity

Exit criteria:

- 11.15 defs, margin markers, `data-look`, scoped ids, rounded-rect classic output, and first-order
  shape classes match representative fresh fixtures.
- Targeted Math fixture remains green.
- The fresh full Flowchart mismatch count is re-measured after the slice.

Primary gates:

- `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --filter upstream_docs_math_flowcharts_001 --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`
- representative targeted filters for class/style and clickable special-shape fixtures
- `cargo nextest run -p merman-render flowchart`

## M2 - Shape, Label, And Cluster Convergence

Exit criteria:

- Markdown/text row DOM matches Mermaid 11.15 for `htmlLabels=false`.
- Special-shape path class surfaces match Mermaid 11.15.
- HTML/`foreignObject` label surfaces and subgraph cluster structure no longer dominate the fresh
  mismatch inventory.
- `flowchart-elk` has an explicit support, skip, or split decision.

Primary gates:

- targeted fresh `compare-svg-xml` filters for each category
- `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`

## M3 - Fresh Full Gate And Stored Baseline Refresh

Exit criteria:

- Supported Flowchart corpus passes fresh Mermaid 11.15 parity comparison.
- Stored Flowchart SVG baselines are regenerated after the fresh gate is green.
- Stored Flowchart parity gate passes.

Primary gates:

- `cargo run -p xtask -- compare-svg-xml --check --diagram flowchart --upstream-root target/upstream-svgs-11-15-flowchart --dom-mode parity --dom-decimals 3`
- `cargo run -p xtask -- gen-upstream-svgs --diagram flowchart --out fixtures/upstream-svgs`
- `cargo run -p xtask -- compare-flowchart-svgs --check-dom --dom-mode parity --dom-decimals 3`

## M4 - Closeout And Umbrella Reintegration

Exit criteria:

- The umbrella M15C-060 evidence points to final Flowchart gates.
- Remaining work is completed, skipped with policy, or split.
- `review-workstream` and `verify-rust-workstream` evidence is recorded before closeout.

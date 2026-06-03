# HPD-080 - Mindmap Visible Signal Boundary

Task: HPD-080 visible rendering defect triage.

## Question

Does the public Mindmap dark-theme smoke count colors that current Mindmap DOM actually consumes?

## Source Audit

- Pinned Mermaid 11.15 `mindmap/styles.ts` emits section rules, root overrides, and neo/data-look
  rules.
- `cScale0` and `cScaleLabel0` target `.section--1`, which is also present on the compact root
  node, but later `.section-root` rules override the root fill/span path in the same stylesheet.
- `gitBranchLabel0` is emitted as `.section-root text`, while current local Mindmap labels in this
  sample are XHTML `span` labels inside `foreignObject`.
- In redux themes, `nodeBorder` is consumed by `.section-root span`; child node section colors are
  consumed through `.section-0` shape, span, and line DOM.

## Outcome

No production renderer defect was found. Updated the representative Mindmap public smoke to count
only current visible surfaces: root `git0`, redux root `nodeBorder` via `span`, and child
`cScale1` / `cScaleLabel1` / `cScaleInv1`.

Added `mindmap_theme_smoke_counts_current_span_and_child_section_dom_as_visible` in
`crates/merman/tests/theme_renderability_smoke.rs`. The test documents the provider/visible
boundary for root-section overrides and native-text CSS.

## Verification

- `cargo fmt` - passed.
- `cargo fmt --check` - passed.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke mindmap_theme_smoke_counts_current_span_and_child_section_dom_as_visible` -
  passed, 1 test run.
- `cargo nextest run -p merman --features render --test theme_renderability_smoke` - passed, 9
  tests run.
- `Get-Content ... CONTEXT.jsonl | ConvertFrom-Json` - passed, 361 JSONL lines parsed.
- `git diff --check` - passed with only the existing `CONTEXT.jsonl` LF/CRLF working-copy warning.

## Residual

Mindmap remains covered for current root/span/section DOM. Do not count `cScale0`,
`cScaleLabel0`, `gitBranchLabel0`, or `data-look` provider rules as visible unless a future fixture
emits matching current DOM and the rule wins the cascade.

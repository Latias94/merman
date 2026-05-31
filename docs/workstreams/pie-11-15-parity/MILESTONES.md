# Pie 11.15 Parity - Milestones

Status: Active
Last updated: 2026-05-31

## M0 - Scope And Evidence Freeze

Exit criteria:

- Upstream Pie authority is named.
- Local behavior gaps are explicit.
- Task order separates baseline ordering from configured variants.

Status: complete.

## M1 - 11.15 Baseline Behavior

Exit criteria:

- Visible slices render in input order.
- Hidden-slice color-domain behavior remains compatible with upstream.
- Pie 11.15 config keys are present in generated defaults.
- `verify-default-config` is green.

Primary gates:

- `cargo nextest run -p merman-render pie`
- `cargo run -p xtask -- verify-default-config`
- `cargo nextest run -p merman-core config`

Status: complete.

## M2 - Configured Rendering

Exit criteria:

- `textPosition` affects label centroid radius.
- Valid `donutHole` emits donut-sector paths; invalid values fall back to solid Pie behavior.
- `legendPosition` controls legend placement and root viewBox dimensions.
- `highlightSlice` emits upstream class names and CSS.

Primary gates:

- `cargo nextest run -p merman-render pie`
- selected `cargo run -p xtask -- compare-pie-svgs --check-dom --dom-mode parity --dom-decimals 3`

Status: in progress. PIE-050 legend placement is complete; highlight classes remain before this
milestone can close.

## M3 - Closeout

Exit criteria:

- All task evidence is recorded.
- Generated default config remains green.
- Pie docs and alignment notes no longer imply unsupported 11.15 Pie config behavior.
- Any remaining parity gaps are split into a follow-on.

Status: not started.

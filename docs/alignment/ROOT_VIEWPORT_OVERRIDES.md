# Root Viewport Overrides (Pinned Mermaid Baseline)

This document explains how fixture-scoped root viewport overrides are maintained for
`parity-root` SVG checks.

## Why This Exists

Some diagrams still have small but persistent root `<svg>` viewport differences in headless mode,
even after layout/renderer parity improvements.

`parity-root` compares:

- `style="max-width: ...px"`
- `viewBox="..."`

To keep regression checks deterministic for the pinned upstream baselines, we keep **version-scoped,
fixture-scoped** overrides.

Baseline version in this repository: Mermaid `@11.16.0`.

Note: the generated override module filenames still use historical suffixes such as
`*_11_12_2.rs` and `*_11_15_0.rs`. The suffixes are provenance labels; their contents are
maintained to match the pinned baseline until each family is regenerated and renamed.

## Override Files

Root viewport override modules live in `crates/merman-render/src/generated/`:

- `c4_root_overrides_11_12_2.rs`
  - `lookup_c4_root_viewport_override(diagram_id)`
- `er_root_overrides_11_12_2.rs`
  - `lookup_er_root_viewport_override(diagram_id)`
- `eventmodeling_root_overrides_11_15_0.rs`
  - `lookup_eventmodeling_root_viewport_override(diagram_id)`
- `flowchart_root_overrides_11_12_2.rs`
  - `lookup_flowchart_root_viewport_override(diagram_id)`
- `mindmap_root_overrides_11_12_2.rs`
  - `lookup_mindmap_root_viewport_override(diagram_id)`
- `pie_root_overrides_11_12_2.rs`
  - `lookup_pie_root_viewport_override(diagram_id)`
- `sankey_root_overrides_11_12_2.rs`
  - `lookup_sankey_root_viewport_override(diagram_id)`
- `state_root_overrides_11_12_2.rs`
  - `lookup_state_root_viewport_override(diagram_id)`
- `timeline_root_overrides_11_12_2.rs`
  - `lookup_timeline_root_viewport_override(diagram_id)`

State diagram also uses text/bbox overrides in:

- `state_text_overrides_11_12_2.rs`

All modules are registered in `crates/merman-render/src/generated/mod.rs`; Mindmap is feature-gated
with `cytoscape-layout`. Architecture, Class, Requirement, Sequence, and GitGraph do not use
generated fixture-scoped root maps. Pie still has a generated root map for the remaining
browser-measurement residuals.

## Where They Are Applied

Overrides are only applied at render time for root viewport attributes and only when the current
`diagram_id` matches a known fixture stem.

Current integration points:

- C4 renderer: `render_c4_diagram_svg`
- ER renderer: `render_er_diagram_svg`
- EventModeling renderer: `render_eventmodeling_diagram_svg`
- Flowchart renderer: `render_flowchart_v2_svg`
- Mindmap renderer: `render_mindmap_diagram_svg`
- Pie renderer: `render_pie_diagram_svg`
- Sankey renderer: `render_sankey_diagram_svg`
- State renderer: `render_state_diagram_v2_svg`
- Timeline renderer: `render_timeline_diagram_svg`

In upstream parity compares, `xtask` sets `diagram_id` to fixture stem, so these keys match.
For normal application rendering (`diagram_id = "merman"` by default), these fixture keys do not
match and no override is applied.

## Update Workflow

1. Reproduce mismatches:

```sh
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

2. Capture upstream root attributes from `fixtures/upstream-svgs/<diagram>/*.svg`:

- `viewBox`
- `style` max-width numeric value

3. Add/update fixture entries in the corresponding `*_root_overrides_11_12_2.rs` file.

4. Re-run diagram compare and global compare:

```sh
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

5. Update `docs/alignment/STATUS.md` with latest totals.

## Guardrails

- Keep overrides **fixture-scoped** and **version-scoped**.
- Do not add broad/global constants that affect unrelated diagrams.
- Store exact upstream strings for `viewBox`/`max-width` to avoid re-rounding drift.
- Prefer real layout/render parity fixes first; use overrides for remaining deterministic gaps.
- Before deleting a pin, capture a disabled-root audit for the affected families without
  `--fail-on-stale`, using explicit, distinct pre-delete `--out` and `--report-dir` paths. Remove
  only its stale candidates, then capture a post-delete audit with distinct paths and
  `--fail-on-stale`; require zero stale entries and runner issues. Compare the exact outside-table
  mismatch key set between the two reports; `--fail-on-stale` does not admit or hide those
  independent mismatches.

## Current Status

Small fixture-scoped root viewport overrides remain in use for the pinned Mermaid baseline. They
exist to pin `viewBox` + `style max-width` when browser `getBBox()` serialization introduces
deterministic drift that is not yet worth globalizing into layout/render logic.

Current root viewport inventory is tracked by
`cargo run -p xtask -- report-overrides --check-no-growth`; run it against the current worktree for
the authoritative total rather than relying on a historical snapshot in this document. Family
compare reports are likewise authoritative for current `parity` and `parity-root` status. Do not
grow these tables before checking whether residuals share a deterministic pinned-baseline root
viewport or measurement-rule change.

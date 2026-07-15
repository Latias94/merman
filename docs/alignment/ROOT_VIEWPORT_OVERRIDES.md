# Root Viewport Overrides (Pinned Mermaid Baseline)

This document explains how fixture-scoped root viewport evidence is maintained for the shared Root
Viewport policy and `parity-root` SVG checks.

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

## Generated Evidence

Root viewport override modules live in `crates/merman-render/src/generated/`:

- `c4_root_overrides_11_12_2.rs`
- `er_root_overrides_11_12_2.rs`
- `eventmodeling_root_overrides_11_15_0.rs`
- `flowchart_root_overrides_11_12_2.rs`
- `mindmap_root_overrides_11_12_2.rs`
- `pie_root_overrides_11_12_2.rs`
- `sankey_root_overrides_11_12_2.rs`
- `state_root_overrides_11_12_2.rs`
- `timeline_root_overrides_11_12_2.rs`

State diagram also uses text/bbox overrides in:

- `state_text_overrides_11_12_2.rs`

The table modules are private implementation details. `generated/root_viewports.rs` is the only
lookup router and requires the typed `RenderFamilyKind`, pinned Mermaid version, and fixture id.
Mindmap remains feature-gated with `cytoscape-layout`. Families without a generated table always
use computed root bounds.

## Where They Are Applied

Generated values are applied only at render time to root viewport attributes, only under
`RootViewportOverridePolicy::ApplyGenerated`, and only when family, pinned baseline, and
`diagram_id` all match. `RootViewportOverridePolicy::ComputedOnly` is the complete alternative and
does not consult generated tables. There is no mutable string patch or request-local explicit root
override path.

Every built-in renderer supplies bounds and root mode through `RootViewportContext` and
`RootViewportSpec`. Root Viewport owns generated lookup, sizing, max-width formatting,
accessibility/root chrome, escaping, and root attribute order. Family renderers do not call table
lookup functions or emit root attributes directly.

In upstream parity compares, `xtask` sets `diagram_id` to fixture stem, so these keys match.
Normal application rendering without an explicit fixture id uses the renderer's family default id;
those ids do not match fixture-scoped keys, so no generated override is applied.

## Update Workflow

1. Reproduce mismatches:

```sh
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

2. Capture upstream root attributes from `fixtures/upstream-svgs/<diagram>/*.svg`:

- `viewBox`
- `style` max-width numeric value

3. Prefer a source-backed bounds, measurement, or sizing fix. Add or update generated evidence only
   when the remaining value is a bounded browser-derived residual, and preserve its upstream
   provenance.

4. Re-run diagram compare and global compare:

```sh
cargo run -p xtask -- compare-<diagram>-svgs --check-dom --dom-mode parity-root --dom-decimals 3
cargo run -p xtask -- compare-all-svgs --check-dom --dom-mode parity-root --dom-decimals 3
```

5. Update `docs/alignment/STATUS.md` with latest totals.

6. Run the governance checks:

```sh
cargo run -p xtask -- report-overrides --check-no-growth
cargo run -p xtask -- audit-root-overrides --fail-on-stale
```

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
the authoritative stored match-arm total. `audit-root-overrides` expands grouped `|` patterns when
it reports fixture keys, so its key count can be larger than the no-growth match-arm budget. Family
compare reports are likewise authoritative for current `parity` and `parity-root` status. Do not
grow these tables before checking whether residuals share a deterministic pinned-baseline root
viewport or measurement-rule change.

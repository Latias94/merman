# Root Viewport Overrides (Mermaid@11.12.2)

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

Baseline version in this repository: Mermaid `@11.12.2`.

## Override Files

All override maps live in `crates/merman-render/src/generated/`:

- `architecture_root_overrides_11_12_2.rs`
  - `lookup_architecture_root_viewport_override(diagram_id)`
- `class_root_overrides_11_12_2.rs`
  - `lookup_class_root_viewport_override(diagram_id)`
- `mindmap_root_overrides_11_12_2.rs`
  - `lookup_mindmap_root_viewport_override(diagram_id)`

State diagram currently uses text/bbox overrides in:

- `state_text_overrides_11_12_2.rs`

All modules are registered in `crates/merman-render/src/generated/mod.rs`.

## Where They Are Applied

Overrides are only applied at render time for root viewport attributes and only when the current
`diagram_id` matches a known fixture stem.

Current integration points:

- Architecture renderer: `render_architecture_diagram_svg`
- Class renderer: `render_class_diagram_v2_svg`
- Mindmap renderer: `render_mindmap_diagram_svg`

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

## Current Status (2026-02-07)

As of 2026-02-07, all root viewport override maps are empty (0 entries) for the pinned Mermaid baseline.
`parity-root` stability is maintained via renderer-level narrow profile calibrations where needed, and the
override maps remain in place as a version-scoped escape hatch for future drift.

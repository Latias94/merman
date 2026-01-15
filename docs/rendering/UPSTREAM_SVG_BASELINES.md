# Upstream SVG Baselines

This document describes how to generate **upstream Mermaid SVG outputs** that act as baselines for
1:1 parity work.

Baseline version: Mermaid `@11.12.2`.

## Why This Exists

Without upstream SVG baselines, it is easy to "visually align by feel" and regress output
in subtle ways (marker ids, viewBox sizing, CSS selectors, etc). Baselines make changes auditable.

## Golden Layers

To make 1:1 parity work tractable, `merman` keeps multiple kinds of goldens:

- Upstream SVG baselines (this doc): the authoritative end-to-end output from Mermaid (via CLI).
- Semantic snapshots: parser output snapshots for `fixtures/**/*.mmd` (generated via
  `cargo run -p xtask -- update-snapshots`).
- Layout golden snapshots: geometry-level snapshots (`*.layout.golden.json`) that validate the
  headless layout model and help localize diffs to layout vs. SVG rendering (see
  `docs/adr/0047-layout-golden-snapshots.md`).

## Recommended Additional Goldens

If we need tighter 1:1 parity coverage beyond ER, extend the golden strategy in these directions:

- **Diagram-by-diagram SVG compare reports** (like `compare-er-svgs`), including:
  - viewBox + width/height deltas
  - marker and defs checks (arrowheads, gradients, filters)
  - optional geometry probes (e.g. parse `<path d>` and compare command sequences at a high level)
- **Error/diagnostics snapshots** for known-invalid inputs (parse errors and runtime render errors),
  including line/column ranges and message text.
- **Security-level snapshots** for sanitization behavior (e.g. `securityLevel` differences, HTML
  label allowlists), to prevent accidental loosening.
- **Theme/style snapshots** that lock the generated CSS blocks for a small set of themes and config
  overrides (prevents silent selector drift).

## Tooling

We use `@mermaid-js/mermaid-cli` pinned under `tools/mermaid-cli/`.
The CLI version and Mermaid version do not always match 1:1, so we use `npm overrides`
to force Mermaid `11.12.2`.

Install:

- `cd tools/mermaid-cli && npm install`

## Generate (ER only)

- `cargo run -p xtask -- gen-upstream-svgs --diagram er`

Outputs to:

- `fixtures/upstream-svgs/er/*.svg`

## Generate (All supported diagrams)

- `cargo run -p xtask -- gen-upstream-svgs --diagram all`

## Compare (ER)

Generate a small report comparing upstream SVGs and the current Rust Stage-B ER SVG output:

- `cargo run -p xtask -- compare-er-svgs`
- Fail the command if marker definitions diverge:
  - `cargo run -p xtask -- compare-er-svgs --check-markers`
- Fail the command if the **SVG DOM** diverges (ignores attribute order/whitespace and rounds
  numeric tokens for comparison):
  - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-decimals 3`
  - Use a looser, structure-only mode while iterating on DOM shape (replaces numeric tokens with
    `<n>`, ignores `data-points`, and ignores `<style>` text):
    - `cargo run -p xtask -- compare-er-svgs --check-dom --dom-mode structure --dom-decimals 3`

## Notes

- The generator passes `--svgId <fixture_stem>` to make the root SVG id deterministic.
- If rendering fails for a fixture, the tool still writes as many SVGs as possible and records
  failures to `fixtures/upstream-svgs/<diagram>/_failures.txt` (the command will exit non-zero).
- We currently store raw upstream SVG outputs. If diff noise becomes an issue, add a normalization
  pass (whitespace + id rewrites) as a follow-up.

## Known Upstream Rendering Failures (as of Mermaid 11.12.2)

- `fixtures/state/upstream_state_parser_spec.mmd`: includes `__proto__`/`constructor` states; Mermaid CLI currently crashes.
- `fixtures/class/upstream_text_label_variants_spec.mmd`: includes a whitespace-only label (`" "`); Mermaid CLI currently fails (NaN transforms / missing SVG in render tree).

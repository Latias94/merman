# Flowchart Root Viewport Parity (Mermaid 11.16)

This note describes the current Flowchart root viewport contract and the focused tools used when
`parity-root` regresses. The pinned authority is Mermaid `11.16.0`; see
`tools/upstreams/REPOS.lock.json` for the exact source revision.

## Ownership

Flowchart owns the content geometry used to derive its bounds. The shared Root Viewport module owns
responsive or fixed sizing, accessibility attributes, root attribute ordering, and final SVG
emission. The canonical `HeadlessRenderer` operation supplies the render environment and selected
text-measurement operations.

Production rendering has no fixture-id lookup, complete-label answer table, root override policy,
or numeric lattice compensation. Vendored measurement data contains generalized font and DOM
operation profiles only. A host measurer remains authoritative when it successfully answers the
requested operation.

## Verification

Run the focused root comparison with the pinned vendored profile:

```sh
cargo run --release -p xtask -- compare-flowchart-svgs \
  --check-dom \
  --dom-mode parity-root \
  --dom-decimals 3 \
  --report-root \
  --text-measurer vendored
```

The report is written to `target/compare/flowchart_report.md`. Omitting `--check-dom` produces a
diagnostic report without making mismatches pass.

The repository-wide release gate uses the same canonical render path:

```sh
cargo run --release -p xtask -- compare-all-svgs \
  --check-dom \
  --dom-mode parity-root \
  --dom-decimals 3 \
  --flowchart-text-measurer vendored
```

## Root Transform Debugging

When a recursive cluster changes the root bounds, inspect the emitted root transforms and geometry
for one fixture:

```sh
cargo run -p xtask -- debug-flowchart-svg-roots --fixture <fixture-stem>
cargo run -p xtask -- debug-flowchart-svg-diff \
  --fixture <fixture-stem> \
  --min-abs-delta 0.5 \
  --max 80
```

These commands report the root `viewBox` and sizing attributes, nested `<g class="root">`
transforms, and cluster geometry. They are diagnostic views only and do not provide an alternate
render path.

## Investigation Order

When `parity-root` fails, compare descendants before changing root policy:

1. If descendants differ, trace parser, label measurement, Dagre/ELK geometry, shape bounds, or
   nested cluster extraction. Root differences are usually downstream effects.
2. If descendants match, compare the family content bounds passed to Root Viewport with upstream
   `setupViewPortForSVG` inputs and title bounds.
3. Classify browser-only text or `getBBox()` residuals narrowly. Do not move them into semantic
   models, add fixture answers, or broaden comparator normalization.

Important Flowchart semantics include Dagre graph margins, empty subgraphs represented as leaf
nodes, self-loop helper geometry, rough-path rendered bounds, title sanitization, and the selected
text-measurement operation. Any correction should be justified by pinned Mermaid source or a
browser measurement profile with explicit capability and fallback behavior.

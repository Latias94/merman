# Flowchart Root Viewport Parity (Mermaid 11.16)

This note describes the current Flowchart root viewport contract and the focused tools used when
`parity-root` regresses. The pinned authority is Mermaid `11.16.1`; see
`tools/upstreams/REPOS.lock.json` for the exact source revision.

## Ownership

Flowchart owns the content geometry used to derive its bounds. The shared Root Viewport module owns
responsive or fixed sizing, accessibility attributes, root attribute ordering, and final SVG
emission. The canonical typed `Renderer` operation supplies the render environment and selected
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

When a recursive cluster changes the root bounds, regenerate one fixture and inspect the emitted
bounds for the local and upstream SVGs:

```sh
cargo run -p xtask -- compare-flowchart-svgs --filter <fixture-stem> --check-dom --dom-mode parity-root
cargo run -p xtask -- debug-svg-bbox --svg <local.svg>
cargo run -p xtask -- debug-svg-bbox --svg <upstream.svg>
```

The compare report records root `viewBox` and sizing differences; `debug-svg-bbox` identifies the
elements that contribute each emitted bound. These are diagnostic views only and do not provide an
alternate render path.

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

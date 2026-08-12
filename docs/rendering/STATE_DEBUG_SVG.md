# State Debug SVG

Use the standing SVG generation and comparison commands to inspect `stateDiagram` output. These
commands exercise the same render path as the parity checks rather than a family-specific debug
renderer.

## Usage

Generate one or more local SVGs:

`cargo run -p xtask -- gen-debug-svgs --diagram state --filter <fixture_stem>`

Outputs are written under `target/debug-svgs/state/` by default.

## Parity-root viewport debugging

When `xtask compare-state-svgs --dom-mode parity-root` fails, it usually indicates a root `<svg>`
viewport delta (`style="max-width: ...px"` and/or `viewBox="..."`), which depends on:

- the layout extents (including labels and clusters), and
- the `svg.getBBox()`-like bounds approximation used to derive the final viewport.

Generate the focused parity report with:

- `cargo run -p xtask -- compare-state-svgs --filter <fixture_stem> --check-dom --dom-mode parity-root`

Then inspect the contributors to either SVG's emitted bounds with:

- `cargo run -p xtask -- debug-svg-bbox --svg <path-to-svg>`

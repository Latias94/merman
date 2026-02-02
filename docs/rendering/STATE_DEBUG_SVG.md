# State Debug SVG

This is a developer tool to visually inspect the **Stage A headless layout** output for
`stateDiagram` (Mermaid `stateDiagram-v2` renderer path).

It is **not** intended to be pixel-perfect Mermaid SVG output.

## Usage (PowerShell)

Render an SVG from a Mermaid input on stdin:

`Get-Content fixtures\\state\\basic.mmd | cargo run -p merman-render --example state_debug_svg > out.svg`

Then open `out.svg` in a browser or an SVG viewer.

## Parity-root viewport debugging

When `xtask compare-state-svgs --dom-mode parity-root` fails, it usually indicates a root `<svg>`
viewport delta (`style="max-width: ...px"` and/or `viewBox="..."`), which depends on:

- the layout extents (including labels and clusters), and
- the `svg.getBBox()`-like bounds approximation used to derive the final viewport.

To get a focused report for a single fixture (including nested `<g class="root" transform="translate(...)">`
scopes), use:

- `cargo run -p xtask -- analyze-state-fixture --fixture <fixture_stem>`

Optional flags:

- `--root <clusterId>`: pick which nested root scope to analyze (defaults to the first nested scope when present)
- `--out <path>`: write the markdown report to a custom location

The command writes the upstream and local SVGs next to the report (under a `svgs/` directory) so you
can open them side-by-side.

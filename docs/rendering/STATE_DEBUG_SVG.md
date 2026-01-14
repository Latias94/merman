# State Debug SVG

This is a developer tool to visually inspect the **Stage A headless layout** output for
`stateDiagram` (Mermaid `stateDiagram-v2` renderer path).

It is **not** intended to be pixel-perfect Mermaid SVG output.

## Usage (PowerShell)

Render an SVG from a Mermaid input on stdin:

`Get-Content fixtures\\state\\basic.mmd | cargo run -p merman-render --example state_debug_svg > out.svg`

Then open `out.svg` in a browser or an SVG viewer.


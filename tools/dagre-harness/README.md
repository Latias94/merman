# Dagre Harness (JS Reference Runner)

This folder contains a small Node.js harness that runs the **exact** Dagre implementation used by
Mermaid `@11.12.2` (`dagre-d3-es@7.0.13`) and snapshots the resulting node/edge coordinates.

It is used by `xtask` to debug `dugong` parity drift at the **layout output** level (nodes/edges),
before it becomes an SVG `viewBox/max-width` mismatch.

## Usage

```sh
node tools/dagre-harness/run.mjs --in target/compare/dagre/<fixture>.input.json --out target/compare/dagre/<fixture>.js.json
```


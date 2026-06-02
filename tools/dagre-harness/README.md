# Dagre Harness (JS Reference Runner)

This folder contains a small Node.js harness that runs the **exact** Dagre implementation used by
the pinned Mermaid CLI toolchain (`tools/mermaid-cli/node_modules/dagre-d3-es`) and snapshots the
resulting node/edge coordinates.

Current pinned package evidence:

- Mermaid baseline: `mermaid@11.15.0` (see `tools/upstreams/REPOS.lock.json`)
- Dagre integration package: `dagre-d3-es@7.0.14`

It is used by `xtask` to debug `dugong` parity drift at the **layout output** level (nodes/edges),
before it becomes an SVG `viewBox/max-width` mismatch.

## Usage

```sh
node tools/dagre-harness/run.mjs --in target/compare/dagre/<fixture>.input.json --out target/compare/dagre/<fixture>.js.json
```

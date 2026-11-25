# Third-Party Notices

This file records source translated, copied, generated, or embedded in `crates/merman-render-charts`.
It is generated from `docs/release/THIRD_PARTY_COMPONENTS.json`.

## d3-path

The Pie SVG renderer translates D3 path rounding and arc serialization, including the 3.1.1 correction for rounded coincident endpoints of large arcs.

- Version: `3.1.1`
- Source: https://github.com/d3/d3-path.git @ `b9ab34592535861e44ce9db03de4db2c2c9c866a`
- Relationship: `modified`, `translated`
- License: `ISC`
- Legal file: `THIRD_PARTY_LICENSES/d3-path/LICENSE`

## d3-sankey

The Sankey layout translates D3's link alignment, relaxation, and collision resolution in Rust.

- Version: `0.12.3`
- Source: https://github.com/d3/d3-sankey.git @ `bb7233ea0004669878d922e55fe83084ef3cb555`
- Relationship: `modified`, `translated`
- License: `BSD-3-Clause`
- Legal file: `THIRD_PARTY_LICENSES/d3-sankey/LICENSE`

## d3-shape

The SVG parity layer translates D3 curve algorithms, including basis, natural, step, cardinal, bump, and Catmull-Rom variants. The current reference graph uses d3-shape 3.2.1, whose curve source is unchanged from this translation baseline.

- Version: `3.2.0`
- Source: https://github.com/d3/d3-shape.git @ `8ec82658454750cfa29efb1e0ea514e3dd9b2297`
- Relationship: `modified`, `translated`
- License: `ISC`
- Legal file: `THIRD_PARTY_LICENSES/d3-shape/LICENSE`

## fmin

The Venn layout kernel translates the fmin Nelder-Mead and conjugate-gradient optimization behavior.

- Version: `0.0.4`
- Source: https://github.com/benfred/fmin.git @ `6b155c9f4a6ecf73ea5d71666da8e5dcd418b18b`
- Relationship: `modified`, `translated`
- License: `BSD-3-Clause`
- Legal file: `THIRD_PARTY_LICENSES/fmin/LICENSE`

## Mermaid

Merman independently implements Mermaid-compatible behavior while translating selected algorithms, generating defaults, copying architecture icon data, and retaining upstream fixtures and snapshots.

- Version: `12.3.0`
- Source: https://github.com/mermaid-js/mermaid.git @ `2086904e76c692331e34a45d7cc3bc30860d040e`
- Relationship: `behavior-reference`, `copied`, `fixtures`, `generated`, `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/mermaid/LICENSE`

## @upsetjs/venn.js

The Venn family uses a modified Rust translation of the venn.js geometry and layout kernel.

- Version: `2.0.0`
- Source: https://github.com/upsetjs/venn.js.git @ `350c835aab4a92a7570963c28f725cf9f6e5f258`
- Relationship: `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/venn-js/LICENSE`

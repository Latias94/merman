# Third-Party Notices

This file records source translated, copied, generated, or embedded in `crates/merman-render-graph`.
It is generated from `docs/release/THIRD_PARTY_COMPONENTS.json`.

## Cytoscape.js

Architecture layout and styling use source-backed Cytoscape behavior and defaults.

- Version: `3.34.4`
- Source: https://github.com/cytoscape/cytoscape.js.git @ `e394aa858b0699a67be8f9ab426823ae1aae7813`
- Relationship: `behavior-reference`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/cytoscape/LICENSE`

## cytoscape.js-fcose

The headless Architecture layout is a modified Rust implementation of FCoSE behavior.

- Version: `2.2.0`
- Source: https://github.com/iVis-at-Bilkent/cytoscape.js-fcose.git @ `78afcf96512a409abc903699277ad616c02dfad9`
- Relationship: `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/cytoscape-fcose/LICENSE`

## Mermaid

Merman independently implements Mermaid-compatible behavior while translating selected algorithms, generating defaults, copying architecture icon data, and retaining upstream fixtures and snapshots.

- Version: `12.4.0`
- Source: https://github.com/mermaid-js/mermaid.git @ `9f93a44a36e46a36b1379f9de72fb1ee5684ee3f`
- Relationship: `behavior-reference`, `copied`, `fixtures`, `generated`, `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/mermaid/LICENSE`

## non-layered-tidy-tree-layout

Merman's Mindmap tidy-tree layout is a modified Rust translation of the non-layered tidy-tree contour and threading algorithm bundled by Mermaid's tidy-tree adapter.

- Version: `2.0.2`
- Source: https://github.com/stetrevor/non-layered-tidy-tree-layout.git @ `63852fb7646eb079ce0e1976044d54545de273e8`
- Relationship: `modified`, `translated`
- License: `MIT`
- Legal file: `THIRD_PARTY_LICENSES/non-layered-tidy-tree-layout/LICENSE`

## ego-tree code in roxmltree

Both roxmltree releases identify their tree structure as derived from ego-tree under ISC. This notice supplements the roxmltree crate licenses listed in the Rust dependency report.

- Version: `roxmltree 0.20.0 and 0.21.2`
- Source: https://github.com/RazrFalcon/roxmltree @ `b04f75f6a0d17c6248de3eb1b1013bf1bbac44ae`
- Relationship: `linked`
- License: `ISC`
- Legal file: `THIRD_PARTY_LICENSES/roxmltree-ego-tree/LICENSE-ISC`

# Mermaid 12.4 development migration

The selected development reference is Mermaid `12.4.0` at
`9f93a44a36e46a36b1379f9de72fb1ee5684ee3f`. The 39 upstream built-in diagram IDs retain their
order. Agentflow accepts `agentflow` as its stable header and continues to accept `agentflow-beta`.
The independent Tree-sitter package keeps its own recorded source baseline. [Reference bundle](../../tools/upstreams/MERMAID_REFERENCE_BUNDLE.json)

The Playground and reference CLI select the same 13 companion packages as the previous graph.
These include parser `2.0.1`, DOMPurify `3.4.17`, ELK `0.12.1`, Cytoscape `3.34.4`,
reference CLI `12.2.0`, Tidy Tree `1.0.1`, and ZenUML Core `3.50.1`. The selected Mermaid
workspace lock and published companion ranges are unchanged. Companion upgrades have their own
behavior and artifact admission. [Selection decision](../../tools/upstreams/MERMAID_SELECTION_DECISION.json)

## Select a layout

Enable `layout-grid` or `layout-layered` with the applicable diagram feature. The Rust builder
accepts `LayoutBackend::Grid` and `LayoutBackend::Layered`. Mermaid source and site configuration
accept the corresponding `layout` name. The features are opt-in for source builds. [Feature guide](../FEATURES.md#svg-without-general-purpose-layout-engines)

Grid placements use signed JavaScript safe integers as track coordinates. Invalid authored
coordinates report `GRID_INVALID_COORDINATE`, with a source span when the parser has one.
Grid labels participate in placement and routing before SVG emission. The renderer shares the
operation's cumulative work and retained-text limits. [Config admission](../alignment/CONFIG_FRONTMATTER_SUPPORT.md#grid-placement-admission)

Grid follows Mermaid 12.4's fixed 20 px terminal trim when checking corridor occupancy. A short
hierarchical terminal segment can produce a reversed occupied interval that affects later routes.
[Pinned Grid router](https://github.com/mermaid-js/mermaid/blob/9f93a44a36e46a36b1379f9de72fb1ee5684ee3f/packages/mermaid/src/rendering-util/layout-algorithms/grid/routerConstraint.ts#L263)

Layered computes measured routes with axis-tagged points. A recoverable Layered failure can retry
with Dagre when the same operation includes and permits `layout-dagre`. The SVG root carries
`data-layout-fallback="dagre"` after a successful retry. The retry shares the first attempt's
work budget, and cancellation or a resource refusal terminates the operation. [Rust embedding](../rendering/RUST_EMBEDDING.md)

The Node and local Web build tools offer private variants for each layout. Release artifacts keep
their declared feature recipes. Match the wrapper and runtime from the same source revision when
using a local variant. [Node source builds](../../platforms/node/README.md#local-grid-and-layered-builds) · [Playground previews](../../platforms/web/README.md#try-grid-or-layered-in-playground)

## Revised SVG geometry

Mermaid 12.4 spreads crowded ELK arrowheads. When markers overlap on one side of a node, the
ELK adapters for Flowchart, State, Class, ER, Requirement, and Usecase can spread the port
contacts. Aligned Swimlane routes can use a straight shaft while keeping their label and node
outline contacts.

The refreshed SVG reference manifests record the capture environment. Browser label wrapping can
vary with the font probe even when the Mermaid versions emit identical SVG on the same host.
Reviewed comparison receipts bind the exact fixture, upstream SVG, local SVG signature, and
comparison modes. [SVG parity contract](../development/CI.md)

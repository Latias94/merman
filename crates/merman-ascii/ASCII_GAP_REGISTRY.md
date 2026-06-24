# ASCII Gap Registry

Status: Active planning registry
Last updated: 2026-06-24

This registry maps remaining ASCII renderer gaps to owning modules, dependencies, and validation
gates. It does not replace the support matrices:

- `FLOWCHART_SUPPORT.md` remains the shipped flowchart support boundary.
- `SEQUENCE_SUPPORT.md` remains the shipped sequence support boundary.
- `STATE_SUPPORT.md` remains the shipped state support boundary.
- `GRAPH_FIXTURE_GAPS.md` and `SEQUENCE_FIXTURE_GAPS.md` remain copied-fixture gap evidence.

Use this file as the first planning context for new ASCII workstreams.

## Registry

| Gap ID | Capability gap | Owning module | Dependencies | Validation gate | Source |
| --- | --- | --- | --- | --- | --- |
| A-TEXT-010 | Full CJK/emoji multi-cell placement across renderers. | `text::StyledLine`, `canvas`, family label/layout modules | Decide how wide cells are represented in `Canvas`; add family-level placement fixtures. | `cargo nextest run -p merman-ascii cjk`; `cargo nextest run -p merman-ascii` | `FLOWCHART_SUPPORT.md`, `SEQUENCE_SUPPORT.md` |
| A-STYLE-010 | Background/fill style semantics for ANSI/HTML output. | `color`, `canvas`, `text::StyledLine`, shared CSS color parsing, graph/state/sequence adapters | Flowchart, state, and sequence node/group/box/rect backgrounds now flow through the renderer while preserving plain output. Remaining work is limited to broader CSS color forms that terminals cannot represent faithfully, especially alpha blending. | `cargo nextest run -p merman-ascii color`; `cargo nextest run -p merman-ascii flowchart sequence state` | ADR 0067; `FLOWCHART_SUPPORT.md`, `SEQUENCE_SUPPORT.md`, `STATE_SUPPORT.md`; 2026-06-23 sequence background closeout |
| A-GRAPH-010 | Flowchart subgraph direction overrides. | `graph::layout`, `graph::routing`, `graph::draw` | Shipped subset now covers nested local-direction overrides for the exercised flowchart combinations, including the current boundary-aware cross-boundary cases. Keep adding explicit cases only when a concrete Mermaid/parser example proves a remaining gap. | `cargo nextest run -p merman-ascii flowchart`; `cargo nextest run -p merman-ascii graph_fixture` | `FLOWCHART_SUPPORT.md`; 2026-06-24 nested override closeout |
| A-GRAPH-020 | Additional uncommon flowchart shapes, icons, images, Markdown/HTML labels, links, and callbacks. | `graph::model`, `graph::label`, `graph::draw`, `graph::from_flowchart_model` | Split by feature family; reject browser-only semantics that cannot be represented in text. | `cargo nextest run -p merman-ascii flowchart`; feature-specific parser/model tests | `FLOWCHART_SUPPORT.md` |
| A-GRAPH-030 | Broader graph route-plan coverage beyond top-down direct routes. | `graph::routing::plan`, `graph::routing`, `graph::routing::path` | Migrate one route family at a time; keep route-plan tests independent of full snapshots. | `cargo nextest run -p merman-ascii flowchart`; route-plan unit tests | `ascii-architecture-deepening` AAD-030 |
| A-SEQ-020 | Empty sequence boxes. | `sequence::validate`, `sequence::boxes`, `sequence::render` | Decide whether empty background regions should render without actor anchors; keep explicit diagnostics until a row-ownership rule exists. | `cargo nextest run -p merman-ascii sequence`; targeted empty-box tests | `SEQUENCE_SUPPORT.md` |
| A-SEQ-030 | Richer sequence actor presentation metadata beyond terminal participant boxes. | `sequence::model`, `sequence::validate`, `sequence::render` | Actor declarations and extended actor types render as participant boxes; actor links are accepted as SVG metadata and omitted from ASCII. Actor properties and any future shape-specific terminal glyphs need explicit support rules. | `cargo nextest run -p merman-ascii sequence`; support-boundary tests | `SEQUENCE_SUPPORT.md` |
| A-STATE-010 | Remaining state presentation metadata and future state shape variants. | `state::adapter`, `graph::model`, `graph::draw`, `graph::routing`, `color` | Start/end, fork/join, choice, notes, links, composite boundary transitions, divider/concurrency regions, and ANSI/HTML node/group backgrounds now have documented terminal behavior. Add future Mermaid state semantics only when the adapter can either render them through graph/text primitives or preserve them as precise unsupported features. | `cargo nextest run -p merman-ascii state`; `cargo nextest run -p merman-ascii` | `STATE_SUPPORT.md`; `docs/plans/2026-06-23-003-refactor-ascii-capability-parity-state-plan.md` |
| A-CLASSER-010 | Dense, cyclic, and more complex class/ER relation graph layouts. | `relation_graph`, `class::render`, `er::render` | Shared relation labels now cover multiline class/ER relationship labels across vertical stacks, same-endpoint lanes, bidirectional same-pair lanes, and layered overlays. `LayeredRelationScene` owns layered box placement, lane draw ordering, route drawing, direction-aware label placement, and shared lane width budgeting so cycle-closing reverse edges and reverse parallel lanes stay visible. When layer reordering still cannot produce a readable deterministic relation drawing, class/ER render explicit relation-summary fallback sections instead of failing. Remaining work is topology depth: decide whether those dense summaries should become a richer grid/adapter without hiding unreadable cases. | `cargo nextest run -p merman-ascii class er` | README shipped matrix; `ASCII_REFERENCE_COMPARISON.md`; class/ER tests |
| A-XY-010 | Richer XYChart layout policy after compact terminal legends. | `xychart::plot`, `xychart::render`; `merman-core::diagrams::xychart`; `merman-render::xychart` | Plot-cell planning is split from row rendering through `xychart::plot`, multi-series legend rows use typed plot titles when present with stable fallbacks for untitled series, compact plot area is configurable through `AsciiRenderOptions`, axis/title/tick visibility flows through the typed display policy, and `showDataLabelOutsideBar` is carried through both ASCII and SVG layout interfaces. Remaining work is richer multi-series data-label placement and terminal-friendly tooltip alternatives. | `cargo nextest run -p merman-core xychart`; `cargo nextest run -p merman-ascii xychart`; `cargo nextest run -p merman-render --test xychart_svg_test` | README XYChart contract; 2026-06-24 series-title and display-policy closeouts |
| A-FAMILY-010 | Additional Mermaid families after state diagrams. | New family adapters plus shared graph/text modules | State diagrams now have a supported subset in `STATE_SUPPORT.md`; choose the next family only after confirming typed model availability and a support boundary. | `cargo nextest run -p merman-ascii render_model`; family-specific tests | ADR 0014, ADR 0065, `STATE_SUPPORT.md` |

## Closeout Discipline

When a gap is implemented:

- update the relevant support matrix first;
- add or update tests named in the validation gate;
- record the closing workstream or commit in this registry;
- remove the gap only when shipped behavior is documented elsewhere.

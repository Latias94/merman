# ASCII Gap Registry

Status: Active planning registry
Last updated: 2026-06-23

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
| A-STYLE-010 | Background/fill style semantics for ANSI/HTML output. | `color`, `canvas`, `text::StyledLine`, `graph::style`, sequence boxes/control, `state::adapter` | Extend style storage beyond foreground-only roles; preserve plain output behavior. | `cargo nextest run -p merman-ascii color`; `cargo nextest run -p merman-ascii flowchart sequence state` | ADR 0067; `FLOWCHART_SUPPORT.md`, `SEQUENCE_SUPPORT.md`, `STATE_SUPPORT.md` |
| A-GRAPH-010 | Flowchart subgraph direction overrides. | `graph::layout`, `graph::routing`, `graph::draw` | Shipped subset: canonical `LR` subgraphs inside canonical `TD` roots, including the current boundary-aware cross-boundary cases. Remaining work is broader local `TD` coverage and nested mixed-direction behavior. | `cargo nextest run -p merman-ascii flowchart subgraph`; `cargo nextest run -p merman-ascii graph_fixture` | `FLOWCHART_SUPPORT.md`; 2026-06-23 boundary-aware cross-boundary subset |
| A-GRAPH-020 | Additional uncommon flowchart shapes, icons, images, Markdown/HTML labels, links, and callbacks. | `graph::model`, `graph::label`, `graph::draw`, `graph::from_flowchart_model` | Split by feature family; reject browser-only semantics that cannot be represented in text. | `cargo nextest run -p merman-ascii flowchart`; feature-specific parser/model tests | `FLOWCHART_SUPPORT.md` |
| A-GRAPH-030 | Broader graph route-plan coverage beyond top-down direct routes. | `graph::routing::plan`, `graph::routing`, `graph::routing::path` | Migrate one route family at a time; keep route-plan tests independent of full snapshots. | `cargo nextest run -p merman-ascii flowchart`; route-plan unit tests | `ascii-architecture-deepening` AAD-030 |
| A-SEQ-020 | Empty sequence boxes. | `sequence::validate`, `sequence::boxes`, `sequence::render` | Decide whether empty background regions should render without actor anchors; keep explicit diagnostics until a row-ownership rule exists. | `cargo nextest run -p merman-ascii sequence`; targeted empty-box tests | `SEQUENCE_SUPPORT.md` |
| A-SEQ-030 | Richer sequence actor presentation metadata beyond terminal participant boxes. | `sequence::model`, `sequence::validate`, `sequence::render` | Actor declarations and extended actor types render as participant boxes; actor links are accepted as SVG metadata and omitted from ASCII. Actor properties and any future shape-specific terminal glyphs need explicit support rules. | `cargo nextest run -p merman-ascii sequence`; support-boundary tests | `SEQUENCE_SUPPORT.md` |
| A-STATE-010 | State dividers and uncommon state node shapes. | `state::adapter`, `graph::model`, `graph::routing`, `color` | State notes now render as terminal note nodes with open note edges. State links are accepted as omitted interaction metadata. State foreground style metadata maps through `graph::style`; composite group transitions attach to group boundaries; fill/background remains A-STYLE-010. Add each remaining state semantic only when the adapter can either render it through graph/text primitives or preserve it as a precise unsupported feature. | `cargo nextest run -p merman-ascii state`; `cargo nextest run -p merman-ascii` | `STATE_SUPPORT.md`; `docs/plans/2026-06-23-003-refactor-ascii-capability-parity-state-plan.md` |
| A-CLASSER-010 | Dense, cyclic, and more complex class/ER relation graph layouts. | `relation_graph`, `class::render`, `er::render` | Extend layered planner or add another relation graph adapter; keep cyclic diagnostics where no deterministic text layout exists. | `cargo nextest run -p merman-ascii class er` | README shipped matrix; class/ER tests |
| A-XY-010 | Richer XYChart legends and scalable terminal plot area. | `xychart::render` or a future `xychart::plot` module | Split plot-area planning from row rendering before adding variable-size output. | `cargo nextest run -p merman-ascii xychart` | README XYChart contract |
| A-FAMILY-010 | Additional Mermaid families after state diagrams. | New family adapters plus shared graph/text modules | State diagrams now have a supported subset in `STATE_SUPPORT.md`; choose the next family only after confirming typed model availability and a support boundary. | `cargo nextest run -p merman-ascii render_model`; family-specific tests | ADR 0014, ADR 0065, `STATE_SUPPORT.md` |

## Closeout Discipline

When a gap is implemented:

- update the relevant support matrix first;
- add or update tests named in the validation gate;
- record the closing workstream or commit in this registry;
- remove the gap only when shipped behavior is documented elsewhere.

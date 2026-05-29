# ASCII Sequence Control Blocks

Status: Active
Last updated: 2026-05-29

## Why This Lane Exists

`merman-ascii` now has a product-grade sequence renderer for the supported message, note,
lifecycle, autonumber, and participant-box subset. The renderer is also split into owner modules,
so the next important sequence gap is no longer file size; it is semantic support for Mermaid
control blocks.

Mermaid sequence control blocks parse through `merman-core` as endpoint-less control signals in the
normal `SequenceDiagramRenderModel.messages` stream. Today the ASCII adapter treats those signals as
unsupported `control messages`, which is correct as a boundary but not enough for users who expect
`loop`, `alt`, `opt`, `par`, `critical`, and `break` diagrams to render.

## Relevant Authority

- ADRs:
  - `docs/adr/0065-ascii-output-boundary.md`
- Current ASCII support docs:
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Completed prerequisite workstreams:
  - `docs/workstreams/ascii-sequence-parity`
  - `docs/workstreams/ascii-sequence-renderer-modularization`
- Parser/model source of truth:
  - `crates/merman-core/src/diagrams/sequence/mod.rs`
  - `crates/merman-core/src/diagrams/sequence/db.rs`
  - `crates/merman-core/src/diagrams/sequence/render_model.rs`
- Existing SVG block reference:
  - `crates/merman-render/src/svg/parity/sequence/block_collection.rs`
  - `crates/merman-render/src/svg/parity/sequence/block_geometry.rs`
  - `crates/merman-render/src/svg/parity/sequence/block_text.rs`
  - `crates/merman-render/src/svg/parity/sequence/blocks.rs`
  - `crates/merman-render/src/svg/parity/sequence/interactions.rs`

## Problem

The current ASCII sequence adapter assumes every drawable event has concrete `from` and `to`
participants, except activation start/end messages. Mermaid control blocks violate that assumption:
their start, separator, and end markers have no drawable endpoints, but they delimit later rows that
must be framed or separated.

If we bolt control blocks straight into row rendering, the renderer will mix block parsing, vertical
layout, row insertion, and frame overlay logic in one place again. The correct next step is to add a
small block-aware render plan between typed-model adaptation and final text painting.

## Target State

`merman-ascii` should render the selected Mermaid sequence control-block subset as deterministic
terminal diagrams:

- `loop`, `opt`, and `break` render as single-section frames around contained rows.
- `alt`/`else`, `par`/`and`, and `critical`/`option` render as sectioned frames with labels and
  horizontal separators.
- Notes and normal messages inside supported blocks contribute to the block height and frame width.
- Unsupported shapes remain explicit diagnostics instead of silent degradation.
- Existing sequence behavior remains stable outside intentional new control-block snapshots.

The implementation should be an ASCII approximation, not an SVG clone. It may reuse the SVG
collector's semantic mapping, but final geometry should follow terminal constraints and the current
ASCII renderer's line-oriented model.

## In Scope

- Typed control-signal inventory and tests for core sequence message types.
- Internal ASCII sequence control-block model/collector.
- Render-plan changes needed to preserve row IDs or row spans for block frames.
- Text frame rendering for:
  - `loop`
  - `opt`
  - `break`
  - `alt` / `else`
  - `par` / `and`
  - `critical` / `option`
- Focused sequence tests and copied/golden fixtures when useful.
- `crates/merman-ascii/SEQUENCE_SUPPORT.md` updates when support status changes.
- This workstream's docs.

## Out Of Scope

- Changing `merman-core` parser or render model contracts in the first implementation pass.
- SVG parity rendering changes.
- Browser font measurement or pixel geometry.
- Full Mermaid styling for control-block background colors.
- `rect` and `par_over` blocks unless explicitly pulled into this lane after the primary block
  subset is stable.
- Actor shape, wrapped actor label, actor link, and message placement support.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Control signals are present in `SequenceDiagramRenderModel.messages` with no endpoints. | High | `sequence/db.rs` calls `add_signal(None, None, msg, signal_type, ...)` for `ControlSignal`. | If false, add parser/model inventory tests before renderer work. |
| SVG `block_collection.rs` is a useful semantic reference for block nesting and section labels. | High | It maps line types 10/11, 12/13/14, 15/16, 19/20/21, 27/28/29, 30/31, and 32 into typed blocks. | If false, derive the collector directly from `merman-core` constants and tests. |
| ASCII should favor stable readable frames over exact upstream/SVG geometry. | High | ADR 0065 defines ASCII output as a renderer boundary with approximation-friendly behavior. | If exact geometry becomes required, split a parity lane instead of overloading this one. |
| A row-span render plan can be introduced without rewriting all message/note rendering at once. | Medium | The renderer now has separate `model`, `layout`, `events`, `notes`, `boxes`, `text`, and `render` modules. | If false, first split row planning from painting before implementing block frames. |

## Architecture Direction

Prefer a minimal internal addition under `crates/merman-ascii/src/sequence/`:

```text
sequence/blocks.rs          existing participant group-box overlays
sequence/control.rs         control-signal types, stack collector, block spans
sequence/plan.rs            optional row plan if block spans need stable row IDs
sequence/render.rs          top-level orchestration and frame insertion/overlay
```

The exact module names may differ if the implementation shows a smaller split is cleaner. The key
architectural rule is that control-block collection should not be hidden inside low-level message
or note row drawing.

## Rendering Direction

The first implementation should use a line-oriented approximation:

- Convert control markers into block spans over already-rendered event rows.
- Compute block horizontal bounds from contained rows and participant range, then clamp to diagram
  width with padding.
- Insert or overlay top, bottom, and separator rows around affected row ranges.
- Put the block keyword and label on the top border or first content row in a way that survives
  ASCII and Unicode charsets.
- Keep nesting deterministic. If nested blocks become visually ambiguous, support one nesting level
  first and return an explicit unsupported diagnostic for deeper nesting.

## Closeout Condition

This lane can close when:

- the selected primary block subset has tests and documented support status,
- unsupported or deferred block forms are explicit in tests/docs,
- sequence focused and package gates pass,
- generated examples can be rendered for manual inspection,
- and any remaining control-block parity debt is split into a smaller follow-on.

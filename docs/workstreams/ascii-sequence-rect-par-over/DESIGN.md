# ASCII Sequence Rect And ParOver Blocks

Status: Active
Last updated: 2026-05-29

## Why This Lane Exists

`ascii-sequence-control-blocks` closed the primary sequence control-block subset for `loop`, `opt`,
`break`, `alt`, `par`, and `critical`. The remaining parser-supported control forms are `rect` and
`par_over`. They still reach `merman-ascii` as endpoint-less control signals and are currently
rejected as `control messages`.

These two forms are small enough to share one follow-on lane, but they have different product
semantics:

- `rect` is a highlighted region in Mermaid/SVG. Terminal text cannot preserve fill color, so ASCII
  should preserve the region and color expression as readable text.
- `par_over` is represented by core as a distinct start signal followed by the normal `par` end
  signal. ASCII should preserve the source keyword instead of silently collapsing it into `par`.

## Relevant Authority

- ADRs:
  - `docs/adr/0065-ascii-output-boundary.md`
- Completed prerequisite workstreams:
  - `docs/workstreams/ascii-sequence-control-blocks`
  - `docs/workstreams/ascii-sequence-renderer-modularization`
- Support docs:
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Parser and model source of truth:
  - `crates/merman-core/src/diagrams/sequence/mod.rs`
  - `crates/merman-core/src/diagrams/sequence_grammar.lalrpop`
  - `crates/merman-core/src/diagrams/sequence/render_model.rs`
- Existing ASCII implementation:
  - `crates/merman-ascii/src/sequence/model.rs`
  - `crates/merman-ascii/src/sequence/control.rs`
  - `crates/merman-ascii/src/sequence/render.rs`
  - `crates/merman-ascii/tests/sequence_model.rs`
- SVG semantic references:
  - `crates/merman-render/src/svg/parity/sequence/block_collection.rs`
  - `crates/merman-render/src/svg/parity/sequence/frames.rs`

## Problem

`rect` and `par_over` are valid Mermaid sequence syntax in `merman-core`, but the ASCII adapter does
not classify their control signals:

- `rect` emits `LINETYPE_RECT_START` (22) and `LINETYPE_RECT_END` (23).
- `par_over` emits `LINETYPE_PAR_OVER_START` (32) and then the normal `LINETYPE_PAR_END` (21).

The current control-frame collector only recognizes the primary block subset. As a result, users get
an unsupported diagnostic for diagrams that are otherwise renderable.

## Target State

`merman-ascii` should render the remaining non-nested sequence control forms deterministically:

- `rect <style>` renders as a single-section region frame around contained rows, using the source
  style/color expression as the frame label.
- `par_over <label>` renders as a single-section frame using the `par_over` keyword and source
  label.
- Existing `par` behavior remains unchanged.
- Notes, activations, lifecycle rows, and participant boxes keep working inside the new frames.
- Empty sections, malformed hand-built ordering, and nested control blocks remain explicit
  diagnostics unless intentionally pulled into this lane.
- Support docs and README state the shipped boundary.

## In Scope

- Executable inventory tests for `rect` and `par_over` core line types.
- ASCII adapter support for `rect` control start/end markers.
- ASCII adapter support for `par_over` start with `par` end.
- Golden-style behavior tests for Unicode and ASCII output.
- Edge policy tests for supported combinations and existing unsupported nested/empty behavior.
- `crates/merman-ascii/SEQUENCE_SUPPORT.md`, README, and workstream docs.
- Manual example output for inspection before closeout.

## Out Of Scope

- ANSI color, terminal background fill, or style interpretation for `rect`.
- Nested control-block rendering.
- Empty control-section rendering.
- SVG renderer changes.
- Parser/model contract changes.
- Exact Mermaid/SVG visual parity.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| `rect` is represented as endpoint-less line types 22/23. | High | `sequence/mod.rs` constants and `sequence_grammar.lalrpop` `RectBlock`. | Add or adjust inventory tests before renderer work. |
| `par_over` starts with line type 32 and ends with line type 21. | High | `sequence_grammar.lalrpop` `ParOverBlock`; SVG collector treats 32 as a `Par`-like start. | Model conversion must support asymmetric start/end matching. |
| Text output should preserve semantics rather than color. | High | ADR 0065 allows terminal approximation and rejects silent semantic loss. | If color becomes required, split ANSI styling behind an option. |
| The existing control-frame renderer can handle both forms with small model changes. | Medium | Existing `SequenceControlKind` already supports single-section frames. | If not, introduce a small frame display descriptor instead of pushing style into row painting. |

## Architecture Direction

Keep the block-aware ownership from the previous lane:

- Add explicit `Rect` and `ParOver` variants or equivalent display metadata in
  `sequence/model.rs`.
- Keep line-type mapping and asymmetric `par_over` end matching in model adaptation, not in low-level
  text painting.
- Reuse `sequence/control.rs` frame rendering where possible.
- Preserve public behavior through parser-to-render tests in `crates/merman-ascii/tests`.

The important boundary is semantic classification first, rendering second. Low-level row rendering
should not need to know Mermaid line type numbers.

## Closeout Condition

This lane can close when:

- `rect` and `par_over` have inventory, rendering, and edge-policy tests,
- unsupported nested/empty/malformed cases stay explicit,
- `SEQUENCE_SUPPORT.md` and README reflect the shipped behavior,
- package and closeout gates pass,
- generated examples can be inspected manually,
- and any remaining parity debt is split or explicitly deferred.

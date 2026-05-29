# ASCII Sequence Renderer Modularization

Status: Active
Last updated: 2026-05-29

## Why This Lane Exists

`merman-ascii` sequence rendering now covers the product-grade subset selected by
`ascii-sequence-parity`, but `crates/merman-ascii/src/sequence.rs` has grown into a single large
module that owns model adaptation, validation, layout, lifecycle state, message rendering, note
rendering, group-box overlays, and text placement utilities.

That shape is acceptable for the completed parity lane, but it is the wrong base for richer Mermaid
sequence control blocks.

## Relevant Authority

- ADRs:
  - `docs/adr/0065-ascii-output-boundary.md`
- Existing docs:
  - `crates/merman-ascii/SEQUENCE_SUPPORT.md`
- Related workstreams:
  - `docs/workstreams/ascii-sequence-parity`

## Problem

Future `loop`/`alt`/`opt`/`par`/`critical`/`break` support needs a block-aware vertical layout
model. Adding that directly to the current single-file renderer would couple control-block planning
to existing message, note, lifecycle, and overlay code, making correctness and regression review
harder than necessary.

## Target State

The sequence renderer keeps the same public API and output behavior, but its internal ownership is
split around stable responsibilities:

- typed model adaptation and unsupported-feature validation,
- internal sequence render model types,
- layout planning and participant/lifeline geometry,
- event rendering for messages, self messages, notes, lifecycle rows, and boxes,
- low-level text placement and overlay utilities.

When this lane closes, adding control blocks should start from an explicit render-plan/layout seam
instead of extending one monolithic file.

## In Scope

- `crates/merman-ascii/src/sequence.rs`
- new internal modules under `crates/merman-ascii/src/sequence/`
- existing public sequence behavior tests and copied upstream sequence golden fixtures
- `crates/merman-ascii/SEQUENCE_SUPPORT.md` only if the internal boundary affects documented
  maintainability notes
- this workstream's docs

## Out Of Scope

- Implementing sequence control blocks.
- Changing ASCII or Unicode output snapshots intentionally.
- Changing `merman-ascii` public API.
- Changing `merman-core` sequence parser/model contracts.
- Rewriting graph ASCII rendering.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| A no-behavior module split is possible before control-block work. | High | `sequence.rs` already has separable model, validation, layout, render, and utility function groups. | If false, stop and record the specific shared-state coupling before changing behavior. |
| Existing sequence behavior tests are strong enough to guard the first extraction. | Medium | `cargo nextest run -p merman-ascii sequence` covers model-driven behavior and upstream golden fixtures. | Add focused regression tests before extraction if a boundary is under-covered. |
| Control blocks need a separate follow-on workstream. | High | `ascii-sequence-parity` closeout split them as `sequence-control-blocks`. | If control blocks are pulled in here, this lane becomes too broad and should be split again. |

## Architecture Direction

Prefer a module tree that deepens the sequence renderer without changing its public surface:

```text
sequence.rs                  public(crate) facade
sequence/model.rs            internal render model and typed-model adapter
sequence/validate.rs         unsupported-feature diagnostics
sequence/layout.rs           participant centers, widths, lifecycle visibility planning
sequence/render.rs           top-level render orchestration
sequence/events.rs           message and self-message rows
sequence/notes.rs            note rows and note wrapping
sequence/boxes.rs            group-box bounds and overlays
sequence/text.rs             sequence-local placement and trim helpers
```

The exact files may differ if extraction shows a smaller boundary is cleaner. The first task should
prefer fewer modules and stable behavior over a large one-shot split.

## Final Module Boundary

As of ASRM-040, the sequence renderer boundary is:

```text
sequence.rs                  public(crate) facade and shared renderer constants
sequence/model.rs            internal render model, typed-model adapter, autonumber handling
sequence/validate.rs         unsupported-feature diagnostics
sequence/layout.rs           participant geometry and lifecycle visibility planning
sequence/render.rs           render orchestration, participant rows, lifelines, overlays
sequence/events.rs           message and self-message rows
sequence/notes.rs            note rows and note wrapping
sequence/boxes.rs            group-box bounds and overlays
sequence/text.rs             sequence-local text placement and trimming helpers
```

This lane intentionally does not implement `loop`, `alt`, `opt`, `par`, `critical`, or `break`.
Those constructs need their own block-aware render-plan workstream.

## Closeout Condition

This lane can close when:

- `sequence.rs` is no longer the sole owner of all sequence responsibilities,
- no public API or output behavior changed unless explicitly documented,
- sequence focused and package gates pass,
- docs record the final module boundary,
- and `sequence-control-blocks` remains a separate follow-on scope.

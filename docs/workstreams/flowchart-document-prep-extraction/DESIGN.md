# Flowchart Document Prep Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

`flowchart/svg_emit.rs` is the largest remaining SVG parity file. It still combines render context
construction, rendered-bounds approximation, root viewport preparation, accessibility metadata, and
the final document shell. Extracting the root SVG document preparation is the next bounded split.

## Relevant Authority

- Existing docs:
  - `docs/rendering/REFACTOR_TODO.md`
  - `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`

## Problem

The root `<svg>` setup is output-sensitive but not flowchart traversal logic. Keeping viewport
formatting, root overrides, root attributes, and accessibility metadata inside `svg_emit.rs` makes
that file harder to audit and increases the chance of accidental root parity drift.

## Target State

`flowchart/svg_emit.rs` keeps flowchart render orchestration and rendered-bounds collection.
`flowchart/document.rs` owns the final viewport numbers, root override application, root SVG attrs,
and accessibility `<title>/<desc>` emission.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/document.rs`.
- Move root viewport quantization/padding, override application, root attrs, and acc title/desc
  handling out of `svg_emit.rs`.
- Preserve exact emitted SVG.
- Record fresh validation evidence.

## Out Of Scope

- Moving edge curve bounds collection.
- Splitting flowchart node or edge rendering.
- Changing generated root viewport overrides.
- Performance benchmarking.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Root document prep can be isolated after content bbox is known. | High | `svg_emit.rs` computes `bbox_min/max` before formatting root attrs. | The module may need a request/result struct with a few more fields. |
| Existing flowchart gates cover root attr and accessibility output. | Medium | Flowchart SVG tests and compare-flowchart DOM gate exist. | Add focused root test before closeout if needed. |
| Keeping edge curve bbox union in `svg_emit.rs` avoids over-broad movement. | High | It shares edge geometry cache and timing counters with render orchestration. | A later lane can extract bounds computation separately. |

## Architecture Direction

Introduce a small document-prep module with request/result structs. It should own formatting and
emission of the root document shell, while receiving already-computed content bounds from
`svg_emit.rs`.

## Closeout Condition

This lane can close when:

- root document preparation compiles from `flowchart/document.rs`,
- flowchart and package validation gates pass,
- evidence is recorded,
- and further flowchart splits are deferred or split into new bounded lanes.

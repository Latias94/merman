# Flowchart Defs Extraction

Status: Complete
Last updated: 2026-05-28

## Why This Lane Exists

Flowchart marker and defs emission currently sits partly in `css.rs` and partly in `svg_emit.rs`.
That couples CSS generation, marker id formatting, edge-derived marker color collection, and root
SVG orchestration even though these are separate concerns.

## Relevant Authority

- `docs/rendering/REFACTOR_TODO.md`
- `docs/rendering/FEARLESS_REFACTORING_SVG_PARITY.md`
- `docs/workstreams/flowchart-document-prep-extraction/`

## Problem

`css.rs` owns `flowchart_markers`, marker color id formatting, extra marker emission, and marker
color collection. This makes the module name misleading and keeps future flowchart `defs/*`
extraction work hidden behind CSS terminology.

## Target State

`flowchart/defs.rs` owns flowchart marker/defs preparation and emission. `svg_emit.rs` asks the
defs module for prepared marker state, emits base markers before the root render, and emits extra
colored markers after the root render to preserve DOM order.

## In Scope

- Add `crates/merman-render/src/svg/parity/flowchart/defs.rs`.
- Move marker id formatting, base marker emission, extra marker color collection, and extra marker
  emission out of `css.rs`.
- Keep emitted SVG byte-for-byte equivalent for covered fixtures.
- Record fresh validation evidence.

## Out Of Scope

- Moving edge class attribute emission.
- Moving edge marker base selection from `edge.rs`.
- Changing root document preparation.
- Performance benchmarking.

## Starting Assumptions

| Assumption | Confidence | Evidence | Consequence if wrong |
| --- | --- | --- | --- |
| Base marker output can move without changing DOM order. | High | `svg_emit.rs` emits it at a single call site before root render. | Keep methods split into base and extra marker emission. |
| Extra marker colors can be prepared before render and emitted after render. | High | Current code collects colors before `render_flowchart_root` and emits after. | Preserve a prepared defs struct with two emission methods. |
| Marker id formatting is a defs concern, not CSS. | High | It produces SVG `marker-*` URL ids and marker defs. | Edge path renderer should call the defs module. |

## Architecture Direction

Introduce a small `FlowchartDefs` value:

- stores the diagram id,
- stores edge-derived extra marker colors,
- emits base markers and extra colored markers in separate methods.

This keeps `svg_emit.rs` as orchestration and makes `css.rs` about style text again.

## Closeout Condition

This lane can close when:

- marker/defs code lives in `flowchart/defs.rs`,
- flowchart parity and package gates pass,
- evidence records the current local machine,
- and follow-on flowchart splits remain tracked in `REFACTOR_TODO.md`.
